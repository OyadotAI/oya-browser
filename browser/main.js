const governance = require('./governance');
/**
 * Oya Browser — Desktop browser with agent scripts built in.
 * Users run this on their machines. Connects to a deployed Oya server.
 * Multi-tab, persistent cookies, real browser — no extension install needed.
 *
 * All user input (click, type, key press, scroll) goes through Chrome DevTools
 * Protocol for full native control. Human-like timing and mouse paths.
 */

const { app, BrowserWindow, BrowserView, ipcMain, session: electronSession, nativeImage, Menu, nativeTheme, dialog, Notification, safeStorage, utilityProcess } = require('electron');
const path = require('path');
const fs = require('fs');
const crypto = require('crypto');
const WebSocket = require('ws');
const { applyTelemetryFlags, applyDomainBlocking } = require('./anonymity/telemetry');
const { buildInjectionScript } = require('./anonymity/inject');
const { createPersonaApplier } = require('./anonymity/apply');
const { LoginState } = require('./login-state');
const { configureProxy, takeProxyBytes, applyDNSLeakPrevention } = require('./anonymity/proxy');
const { ProfileStore } = require('./anonymity/profile-store');
const { shellLayout } = require('./shell-layout.cjs');
const { createControlState } = require('./control-state.cjs');
// Branding must not move existing cookies, profiles, or saved settings.
const desktopUserDataPath = app.getPath('userData');
app.setName('Oya Browser');
app.setPath('userData', desktopUserDataPath);

// Shell appearance is independent of the websites' preferred color scheme.
if (process.env.OYA_USER_DATA_DIR) app.setPath('userData', path.resolve(process.env.OYA_USER_DATA_DIR));
// CDP for automation harnesses. Off unless asked for: whoever reaches this port
// owns the browser. Chromium listens one port up on loopback; cdp-front-door.js
// owns the public port and shows harnesses only real, protected tabs. No
// remote-allow-origins — CDP clients send no Origin, and allowing one would
// let any web page on the machine drive it.
const CDP_PORT = Number(process.env.OYA_REMOTE_DEBUGGING_PORT) || 0;
const CDP_RELAY_TOKEN = crypto.randomBytes(32).toString('hex');
app.commandLine.appendSwitch('remote-debugging-port', CDP_PORT ? String(CDP_PORT + 1) : '0');

// Prevent crashes from unhandled errors
process.on('uncaughtException', (err) => {
  console.error('[oya] Uncaught exception:', err.message);
});
process.on('unhandledRejection', (reason) => {
  console.error('[oya] Unhandled rejection:', reason?.message || reason);
});

// Apply telemetry + DNS leak prevention flags before app is ready
applyTelemetryFlags(app);
applyDNSLeakPrevention(app);

// Set dock icon on macOS (needed for dev mode — built app uses icon from package.json)
//
// icon_1024, not icon.png: the latter is the 6250x6250 master electron-builder
// resizes at build time. Handing it to nativeImage decodes 156MB per
// representation, and macOS makes several — it was 596MB of CG image data in
// the main process, most of this app's memory, for a dock icon.
if (process.platform === 'darwin') {
  const iconPath = path.join(__dirname, 'build', 'icon_1024.png');
  if (fs.existsSync(iconPath)) {
    app.whenReady().then(() => {
      app.dock.setIcon(nativeImage.createFromPath(iconPath));
    });
  }
}

// ─── Config ───

const CONFIG_DEFAULTS = {
  serverUrl: 'ws://localhost:3100/ws',
  apiKey: '',
  browserName: `Oya Browser ${process.platform}`,
  activeProfileId: null,
};

let activeProfile = null;
let profileStore = null;
let loginState = null;

let config = { ...CONFIG_DEFAULTS };
let configPath = null;

function loadConfig() {
  configPath = path.join(app.getPath('userData'), 'config.json');
  try {
    const data = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    config = { ...CONFIG_DEFAULTS, ...data };
  } catch {}
  // Env vars override file config (for Docker / headless use)
  if (process.env.OYA_SERVER_URL) config.serverUrl = process.env.OYA_SERVER_URL;
  if (process.env.OYA_API_KEY) config.apiKey = process.env.OYA_API_KEY.split(',')[0].trim();
  if (process.env.OYA_BROWSER_NAME) config.browserName = process.env.OYA_BROWSER_NAME;
  // Provisioned sandboxes are given their id up front so the server can
  // correlate the sandbox it created with the browser that enrolls.
  if (process.env.OYA_BROWSER_ID) browserId = process.env.OYA_BROWSER_ID;
  if (process.env.OYA_PERSONA) config.persona = process.env.OYA_PERSONA;
  if (process.env.OYA_PROVIDER) config.provider = process.env.OYA_PROVIDER;
}

function saveConfig() {
  // 0600: config holds apiKey, which is a control-plane credential for the
  // whole project. Every other credential-bearing write in this codebase is
  // explicit about the mode; the default 0644 leaves it readable by any other
  // local account on a Linux host or in a container.
  try {
    fs.writeFileSync(configPath, JSON.stringify(config, null, 2), { mode: 0o600 });
    fs.chmodSync(configPath, 0o600);   // a file written before this was 0644
  } catch {}
}

// ─── Scripts ───

const analyzerScript = fs.readFileSync(path.join(__dirname, 'scripts', 'analyzer.js'), 'utf8');

// ─── Chrome DevTools Protocol ───

const CDP_VERSION = '1.3';

// A CDP command that never answers must not strand the tab that is waiting on
// it. Cloud browsers hit exactly that: setupTabCDP never resolved, so the
// initial loadURL chained after it never ran — the tab sat on its start URL
// with the title it was created with, and everything that waited on the tab
// waited forever. Going ahead unprotected is bad; never loading a page is worse,
// and the fail() above says so loudly either way.
const CDP_SETUP_TIMEOUT = 10000;

function cdpAttach(view) {
  if (!view || view.webContents.isDestroyed()) return null;
  const dbg = view.webContents.debugger;
  if (!dbg.isAttached()) {
    try { dbg.attach(CDP_VERSION); } catch {}
  }
  return dbg;
}

async function cdp(view, method, params = {}) {
  const dbg = cdpAttach(view);
  if (!dbg) throw new Error('View is destroyed');
  return dbg.sendCommand(method, params);
}

// ─── Isolated world ───
//
// The analyzer runs in its own JS world, not the page's. The page can then
// neither see our globals (window.analyzePage was a one-line, 100%-precision
// identifier for this product) nor reach into them. The name is randomised per
// process so it is not a constant to match on either.
//
// An isolated world shares the DOM but has its own globals, so it also gets the
// UNPATCHED getBoundingClientRect — which is why the analyzer no longer needs a
// flag to switch the fingerprint noise off while it measures.

const { RecordingChannel } = require('./scripts/recording.cjs');
const { Workspace } = require('./scripts/workspace.cjs');
const { DraftStore } = require('./scripts/draft-store.cjs');
const { normalizeStep, normalizeDraft } = require('./scripts/workflow.cjs');
const { redact } = require('./scripts/diagnostics.cjs');
let workspace;
const recordingTabMap = new Map();
function recordingTab(id) {
  if (!recordingTabMap.has(id)) { const used = new Set([...recordingTabMap.values(), ...recordedSteps.map(s => s.tab)]); let n = 1; while (used.has('tab-' + n)) n++; recordingTabMap.set(id, 'tab-' + n); }
  return recordingTabMap.get(id);
}
let captureSignature = '';
function emitRecording() {
  sendToRenderer('recorded-steps', { recording, steps: recordedSteps });
  const signature = JSON.stringify([recording, recordedSteps, [...recordedSecrets]]);
  if (workspace && signature !== captureSignature) { captureSignature = signature; workspace.capture(recordedSteps, recordedSecrets, recording); }
}

const ISOLATED_WORLD = 'w' + require('crypto').randomBytes(8).toString('hex');
const worldContexts = new WeakMap(); // view -> executionContextId

/**
 * Create (or recreate) the isolated world for this view's main frame and load
 * the analyzer into it. Page.createIsolatedWorld returns the context id
 * directly, so this needs no Runtime.enable — that domain is a detection
 * vector. Only an active recording enables it to receive captured events.
 */
async function ensureWorld(view, { force = false } = {}) {
  if (!force && worldContexts.has(view)) return worldContexts.get(view);
  const { frameTree } = await cdp(view, 'Page.getFrameTree');
  const frameId = frameTree.frame.id;
  const { executionContextId } = await cdp(view, 'Page.createIsolatedWorld', {
    frameId, worldName: ISOLATED_WORLD, grantUniveralAccess: true,
  });
  worldContexts.set(view, executionContextId);

  // A fresh tag attribute per document, so the marks the analyzer leaves on the
  // DOM are not a constant any MutationObserver can match on.
  const attr = 'data-' + require('crypto').randomBytes(4).toString('hex');
  await cdp(view, 'Runtime.evaluate', {
    // RecordingChannel arms new documents while a recording is active.
    expression: analyzerScript.replace('__OYA_ATTR__', attr).replace('__OYA_RECORD__', 'false'),
    contextId: executionContextId,
    returnByValue: true,
  });
  return executionContextId;
}

/**
 * Evaluate in the isolated world. Retries once against a fresh world, because
 * a navigation between calls invalidates the context id.
 */
async function worldEval(view, expression, { retry = true } = {}) {
  const contextId = await ensureWorld(view);
  let res;
  try {
    res = await cdp(view, 'Runtime.evaluate', {
      expression, contextId, returnByValue: true, awaitPromise: true,
    });
  } catch (err) {
    if (retry && /context|Cannot find/i.test(err.message || '')) {
      await ensureWorld(view, { force: true });
      return worldEval(view, expression, { retry: false });
    }
    throw err;
  }
  if (res.exceptionDetails) {
    throw new Error(res.exceptionDetails.exception?.description || res.exceptionDetails.text || 'Evaluation failed');
  }
  return res.result?.value;
}

// ── Key definitions (CDP Input.dispatchKeyEvent format) ──

const KEY_DEFS = {
  'Enter':      { key: 'Enter',      code: 'Enter',      keyCode: 13 },
  'Tab':        { key: 'Tab',        code: 'Tab',        keyCode: 9  },
  'Backspace':  { key: 'Backspace',  code: 'Backspace',  keyCode: 8  },
  'Delete':     { key: 'Delete',     code: 'Delete',     keyCode: 46 },
  'Escape':     { key: 'Escape',     code: 'Escape',     keyCode: 27 },
  'ArrowUp':    { key: 'ArrowUp',    code: 'ArrowUp',    keyCode: 38 },
  'ArrowDown':  { key: 'ArrowDown',  code: 'ArrowDown',  keyCode: 40 },
  'ArrowLeft':  { key: 'ArrowLeft',  code: 'ArrowLeft',  keyCode: 37 },
  'ArrowRight': { key: 'ArrowRight', code: 'ArrowRight', keyCode: 39 },
  'Home':       { key: 'Home',       code: 'Home',       keyCode: 36 },
  'End':        { key: 'End',        code: 'End',        keyCode: 35 },
  'PageUp':     { key: 'PageUp',     code: 'PageUp',     keyCode: 33 },
  'PageDown':   { key: 'PageDown',   code: 'PageDown',   keyCode: 34 },
  'Space':      { key: ' ',          code: 'Space',      keyCode: 32, text: ' ' },
};

function keyDef(ch) {
  if (KEY_DEFS[ch]) return { ...KEY_DEFS[ch] };
  const upper = ch.toUpperCase();
  const isLetter = /^[a-zA-Z]$/.test(ch);
  const isDigit = /^[0-9]$/.test(ch);
  return {
    key: ch,
    code: isLetter ? 'Key' + upper : isDigit ? 'Digit' + ch : '',
    keyCode: isLetter ? upper.charCodeAt(0) : isDigit ? ch.charCodeAt(0) : 0,
    text: ch,
    shift: ch !== ch.toLowerCase() && ch === upper,
  };
}

// ── Human-like timing ──

function typingDelay(ch, prev) {
  let ms = 8 + Math.random() * 18;                          // 8-26ms base (~200-300 WPM)
  if (prev === ' ') ms += 5 + Math.random() * 15;           // small word boundary pause
  if (!/[a-zA-Z0-9 ]/.test(ch)) ms += 5 + Math.random() * 10; // special char
  if (Math.random() < 0.02) ms += 30 + Math.random() * 50;    // 2% micro-hesitation
  return ms;
}

const sleep = (ms) => new Promise(r => setTimeout(r, ms));

// ── CDP keyboard ──

async function cdpKeyDown(view, def, modifiers = 0) {
  const isChar = !!def.text;
  await cdp(view, 'Input.dispatchKeyEvent', {
    type: isChar ? 'keyDown' : 'rawKeyDown',
    modifiers,
    windowsVirtualKeyCode: def.keyCode,
    nativeVirtualKeyCode: def.keyCode,
    key: def.key, code: def.code,
    text: isChar ? def.text : undefined,
    unmodifiedText: isChar ? def.text : undefined,
  });
}

async function cdpKeyUp(view, def, modifiers = 0) {
  await cdp(view, 'Input.dispatchKeyEvent', {
    type: 'keyUp',
    modifiers,
    windowsVirtualKeyCode: def.keyCode,
    nativeVirtualKeyCode: def.keyCode,
    key: def.key, code: def.code,
  });
}

async function cdpPressKey(view, key, modifiers = 0) {
  const def = keyDef(key);
  await cdpKeyDown(view, def, modifiers);
  await sleep(20 + Math.random() * 30);
  await cdpKeyUp(view, def, modifiers);
}

async function cdpTypeText(view, text) {
  let prev = '';
  for (const ch of text) {
    const def = keyDef(ch);
    const mods = def.shift ? 8 : 0; // Shift = 8
    await cdpKeyDown(view, def, mods);
    await cdpKeyUp(view, def, mods);
    await sleep(typingDelay(ch, prev));
    prev = ch;
  }
}

async function cdpSelectAll(view) {
  const mod = process.platform === 'darwin' ? 4 : 2; // Meta=4, Ctrl=2
  await cdpPressKey(view, 'a', mod);
}

async function cdpClearField(view) {
  await cdpSelectAll(view);
  await sleep(10 + Math.random() * 15);
  await cdpPressKey(view, 'Backspace');
  await sleep(10 + Math.random() * 15);
}

// ── CDP mouse (human-like Bézier paths) ──

let mouseX = 0, mouseY = 0;

function bezier(t, p0, p1, p2, p3) {
  const u = 1 - t;
  return u * u * u * p0 + 3 * u * u * t * p1 + 3 * u * t * t * p2 + t * t * t * p3;
}

function mousePath(x1, y1, x2, y2) {
  const dist = Math.hypot(x2 - x1, y2 - y1);
  const steps = Math.max(5, Math.min(30, Math.round(dist / 25)));
  // Random control points create a natural arc
  const jitter = dist * 0.3;
  const cp1x = x1 + (x2 - x1) * 0.25 + (Math.random() - 0.5) * jitter;
  const cp1y = y1 + (y2 - y1) * 0.25 + (Math.random() - 0.5) * jitter;
  const cp2x = x1 + (x2 - x1) * 0.75 + (Math.random() - 0.5) * jitter * 0.6;
  const cp2y = y1 + (y2 - y1) * 0.75 + (Math.random() - 0.5) * jitter * 0.6;
  const pts = [];
  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    // Ease-out: move fast at start, decelerate near target (like a real hand)
    const te = 1 - Math.pow(1 - t, 2);
    pts.push({
      x: Math.round(bezier(te, x1, cp1x, cp2x, x2)),
      y: Math.round(bezier(te, y1, cp1y, cp2y, y2)),
    });
  }
  return pts;
}

async function cdpMouseMove(view, toX, toY) {
  const pts = mousePath(mouseX, mouseY, toX, toY);
  for (const pt of pts) {
    await cdp(view, 'Input.dispatchMouseEvent', {
      type: 'mouseMoved', x: pt.x, y: pt.y,
    });
    await sleep(4 + Math.random() * 12);
  }
  mouseX = toX; mouseY = toY;
}

async function cdpClick(view, x, y) {
  const ix = Math.round(x), iy = Math.round(y);
  await cdpMouseMove(view, ix, iy);
  // Brief hover before click
  await sleep(10 + Math.random() * 20);
  await cdp(view, 'Input.dispatchMouseEvent', {
    type: 'mousePressed', x: ix, y: iy,
    button: 'left', clickCount: 1, buttons: 1,
  });
  // Brief hold before release
  await sleep(10 + Math.random() * 20);
  await cdp(view, 'Input.dispatchMouseEvent', {
    type: 'mouseReleased', x: ix, y: iy,
    button: 'left', clickCount: 1,
  });
}

async function cdpScroll(view, x, y, deltaX, deltaY) {
  await cdp(view, 'Input.dispatchMouseEvent', {
    type: 'mouseWheel', x, y, deltaX, deltaY,
  });
}

// ── CDP page helpers ──

async function cdpEval(view, expression) {
  const res = await cdp(view, 'Runtime.evaluate', {
    expression, returnByValue: true, awaitPromise: true,
  });
  if (res.exceptionDetails) throw new Error(res.exceptionDetails.text || 'Eval failed');
  return res.result?.value;
}

// ─── Window & Tabs ───

let mainWindow = null;
let browsingMode = false;
let devPanelOpen = false;
let devPanelWidth = 360;
const shellOverlays = new Set();
let controlShield;
const backdropWaiters = new Map();
const controlPopups = new Set();
const desktopControl = createControlState({ send: message => wsSend(message), changed: state => {
  sendToRenderer('control-state', state);
  if (recording && recordingOrigin === 'desktop' && !state.interactive && recordingCutoff === Infinity) { recordingCutoff = Date.now(); queueRecording(stopRecording).catch(() => {}); }
  for (const id of ['browser-new-tab', 'browser-close-tab', 'browser-reload']) {
    const item = Menu.getApplicationMenu()?.getMenuItemById(id);
    if (item) item.enabled = state.interactive;
  }
  for (const popup of controlPopups) if (!popup.isDestroyed()) popup.setEnabled(state.interactive);
  if (mainWindow && !mainWindow.isDestroyed()) syncControlShield();
} });
function requireHumanControl() {
  if (!desktopControl.snapshot().interactive) throw new Error('Take control before interacting with this page');
}
function prepareControlShield() {
  if (controlShield) return;
  controlShield = new BrowserView({ webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false } });
  controlShield.setBackgroundColor('#00000000');
  controlShield.webContents.loadFile(path.join(__dirname, 'renderer/control-shield.html'));
  installShellShortcuts(controlShield.webContents);
}
function syncControlShield() {
  if (!mainWindow || mainWindow.isDestroyed()) return;
  if (!browsingMode || shellOverlays.size || desktopControl.snapshot().interactive) {
    if (controlShield) mainWindow.removeBrowserView(controlShield);
    return;
  }
  prepareControlShield();
  const view = getActiveView();
  if (view) {
    controlShield.setBounds(view.getBounds());
    if (!mainWindow.getBrowserViews().includes(controlShield)) mainWindow.addBrowserView(controlShield);
    mainWindow.setTopBrowserView(controlShield);
    if (view.webContents.isFocused()) mainWindow.webContents.focus();
  }
}

/** @type {{ id: number, view: BrowserView, title: string, url: string }[]} */
const tabs = [];
let activeTabId = null;
let nextTabId = 1;

// ─── Cookie Sync ───

let applyingCookieSync = false;

function getPartitionName() {
  if (activeProfile) return `persist:oya-${activeProfile.id}`;
  return 'persist:oya-browser';
}

function getBrowserSession() {
  return electronSession.fromPartition(getPartitionName());
}

/** Dump all cookies to the server for pool sync. */
async function dumpCookies() {
  if (!ws || ws.readyState !== WebSocket.OPEN) return;
  try {
    const cookies = await getBrowserSession().cookies.get({});
    const slim = cookies.map(c => ({
      name: c.name, value: c.value, domain: c.domain, path: c.path,
      secure: c.secure, httpOnly: c.httpOnly, sameSite: c.sameSite || 'unspecified',
      expirationDate: c.expirationDate,
      hostOnly: c.hostOnly,
    }));
    wsSend({ type: 'cookie_dump', cookies: slim });
  } catch (e) {
    console.log('[oya] Cookie dump failed:', e.message);
  }
}

/** Apply a full cookie jar from the server. */
async function applyCookieSync(cookies) {
  if (!Array.isArray(cookies)) return;
  applyingCookieSync = true;
  const jar = getBrowserSession().cookies;
  // One at a time meant a round trip per cookie, and auth_ok awaits this from
  // inside the serialized message queue — a real jar froze the app on sign-in.
  const results = await Promise.allSettled(cookies.map(async (c) => jar.set({
    url: `http${c.secure ? 's' : ''}://${c.domain.replace(/^\./, '')}${c.path || '/'}`,
    name: c.name,
    value: c.value,
    ...(c.hostOnly || !c.domain.startsWith('.') ? {} : { domain: c.domain }),
    path: c.path || '/',
    secure: c.secure || false,
    httpOnly: c.httpOnly || false,
    sameSite: ({ Strict: 'strict', Lax: 'lax', None: 'no_restriction' })[c.sameSite] || c.sameSite || 'unspecified',
    expirationDate: Number(c.expirationDate ?? c.expires) > 0 ? Number(c.expirationDate ?? c.expires) : undefined,
  })));
  const applied = results.filter((r) => r.status === 'fulfilled').length;
  applyingCookieSync = false;
  console.log(`[oya] Cookie sync applied: ${applied}/${cookies.length}`);
}

/** Apply an incremental cookie update from the server. */

// ── Pull-based cookie sync ──
//
// The server no longer pushes every cookie change to every browser in the pool
// — that was O(pool size) per change. Instead each browser asks for the hosts
// it is about to visit, so sync cost tracks navigations rather than the square
// of the fleet size. Outbound changes are batched for the same reason.

const COOKIE_PULL_TTL = 30000;   // re-pull a host at most this often
const COOKIE_PULL_TIMEOUT = 3000; // never block a navigation longer than this
const COOKIE_FLUSH_MS = 2000;

const pulledAt = new Map();
const pendingPulls = new Map();
let pullSeq = 0;

function hostFor(url) {
  try {
    return new URL(/^https?:\/\//i.test(url) ? url : 'https://' + url).hostname;
  } catch { return null; }
}

/** Fetch this host's cookies from the pool before navigating to it. */
function pullCookiesFor(url, { force = false } = {}) {
  const host = hostFor(url);
  if (!host || !ws || ws.readyState !== WebSocket.OPEN || !wsReady) return Promise.resolve();
  if (!force && Date.now() - (pulledAt.get(host) || 0) < COOKIE_PULL_TTL) return Promise.resolve();
  if (pulledAt.size > 500) pulledAt.clear();
  pulledAt.set(host, Date.now());

  const pullId = `p${++pullSeq}`;
  return new Promise((resolve) => {
    let settled = false;
    // The eager pulledAt.set above dedupes concurrent navigations to this host.
    // Only a real answer may keep it: a pull that timed out synced nothing, and
    // leaving the stamp in place suppressed every retry for the whole TTL.
    const finish = (answered = false) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      pendingPulls.delete(pullId);
      if (answered) pulledAt.set(host, Date.now());
      else pulledAt.delete(host);
      resolve();
    };
    const timer = setTimeout(() => finish(false), COOKIE_PULL_TIMEOUT);
    pendingPulls.set(pullId, finish);
    try {
      if (!wsSend({ type: 'cookie_pull', domains: [host], pullId })) finish();
    } catch { finish(); }
  });
}

/** Start listening for local cookie changes and forward them in batches. */
let cookieBatch = new Map();
let cookieFlushTimer = null;

/** Forget queued changes without sending them: used when the persona changes. */
function dropPendingCookieChanges() {
  clearTimeout(cookieFlushTimer);
  cookieFlushTimer = null;
  cookieBatch = new Map();
}

function flushCookieChanges() {
  cookieFlushTimer = null;
  if (!cookieBatch.size) return;
  const changes = [...cookieBatch.values()];
  cookieBatch = new Map();
  if (!ws || ws.readyState !== WebSocket.OPEN || !wsReady) return;
  wsSend({ type: 'cookie_changed', changes });
}

let watchedCookies = null, cookieListener = null;
function startCookieChangeListener() {
  if (watchedCookies && cookieListener) watchedCookies.removeListener('changed', cookieListener);
  watchedCookies = getBrowserSession().cookies;
  cookieListener = (event, cookie, cause, removed) => {
    if (applyingCookieSync) return;
    // Only forward explicit changes — ignore overwrite (intermediate removal
    // when a cookie is replaced), expired, and evicted events to prevent
    // feedback loops between browsers in the pool.
    if (cause !== 'explicit') return;
    // Keyed so a cookie rewritten repeatedly inside one window collapses to
    // its final value instead of sending every intermediate step.
    cookieBatch.set(`${cookie.domain}|${cookie.path}|${cookie.name}`, {
      removed,
      cookie: {
        name: cookie.name, value: cookie.value, domain: cookie.domain, path: cookie.path,
        secure: cookie.secure, httpOnly: cookie.httpOnly, sameSite: cookie.sameSite || 'unspecified',
        expirationDate: cookie.expirationDate,
        hostOnly: cookie.hostOnly,
      },
    });
    if (cookieBatch.size >= 200) { clearTimeout(cookieFlushTimer); flushCookieChanges(); return; }
    if (!cookieFlushTimer) cookieFlushTimer = setTimeout(flushCookieChanges, COOKIE_FLUSH_MS);
  };
  watchedCookies.on('changed', cookieListener);
}

// ─── WebSocket ───

let ws = null;
let wsReady = false;

/**
 * Send if the socket is up, otherwise drop it. A browser loses its connection
 * for ordinary reasons — sleep, wifi, a server rollout — and a send that
 * throws from inside a message handler used to take the whole session down
 * with it rather than waiting for the reconnect that was already scheduled.
 */
function wsSend(payload) {
  if (!ws || ws.readyState !== WebSocket.OPEN) return false;
  try {
    ws.send(JSON.stringify(payload));
    return true;
  } catch {
    return false;
  }
}
let browserId = null;
let reconnectTimer = null;
let reconnectAttempts = 0;
let pingInterval = null;
let missedPongs = 0;

// ─── Server Fingerprint ───

/**
 * Apply a fingerprint profile received from the server.
 * The server generates the profile from the API key and sends it on auth_ok.
 * This guarantees every browser with the same API key gets the exact same
 * fingerprint — the server is the single source of truth.
 */
async function applyServerFingerprint(profile, cookies = []) {
  if (!profile?.id) return;

  // WebContents partitions are immutable. Rebuild views when the profile
  // changes so the tabs, listener and cookie exporter all use the same jar.
  const switched = activeProfile?.id !== profile.id;
  const reopen = switched ? tabs.map((tab) => tab.url || 'about:blank') : [];
  if (switched) {
    // Drop the pending batch rather than flushing it. Those changes were seen
    // under the previous persona, but this socket is already authenticated as
    // the new one, so the server would file another identity's cookies in this
    // persona's jar — planting a session captured on one device into a jar
    // that a different device will replay, which is what makes a site demand a
    // fresh login. The old jar keeps them; its partition is untouched.
    dropPendingCookieChanges();
    while (tabs.length) closeTab(tabs[0].id, { keepOne: false });
    pulledAt.clear();
  }

  // Persist it so it survives restarts (and loads before reconnect)
  if (profileStore) {
    profileStore.save(profile);
    profileStore.setActiveId(profile.id);
  }

  activeProfile = profile;
  config.activeProfileId = profile.id;
  saveConfig();

  // Re-setup session with the new fingerprint (user-agent, proxy, headers)
  await setupBrowserSession();
  startCookieChangeListener();
  await applyCookieSync(cookies);
  for (const url of reopen) createTab(url);

  // Re-inject into all open tabs so they pick up the new fingerprint
  for (const tab of tabs) {
    if (!tab.view.webContents.isDestroyed()) {
      if (!switched) setupTabCDP(tab.view);
    }
  }

  // Notify renderer so the fingerprint debug bar updates
  sendToRenderer('fingerprint-changed', {
    id: profile.id,
    platform: profile.navigator.platform,
    hardwareConcurrency: profile.navigator.hardwareConcurrency,
    deviceMemory: profile.navigator.deviceMemory,
    screen: `${profile.screen.width}x${profile.screen.height}`,
    dpr: profile.screen.devicePixelRatio,
    gpu: profile.webgl.unmaskedRenderer,
    timezone: profile.timezone,
    locale: profile.locale,
    fonts: profile.fonts.available.length,
    canvasNoise: profile.canvas.noiseSeed.toFixed(6),
    audioNoise: profile.audio.noiseSeed.toFixed(6),
  });
}

// ─── Session Setup ───

/** Configure the persistent browser session — user-agent, cookies, privacy. */
async function setupBrowserSession() {
  const ses = getBrowserSession();

  // Telemetry blocking is handled by Chromium flags (applyTelemetryFlags).
  // Domain-level blocking via onBeforeRequest was removed — it interfered
  // with normal page loads and handler stacking on session reuse.

  // ── User-Agent: strip Electron/oya-browser tokens, match profile platform ──
  const defaultUA = ses.getUserAgent();
  let cleanUA = defaultUA
    .replace(/\s*Electron\/[\d.]+/, '')
    .replace(/\s*oya-browser\/[\d.]+/i, '');

  const chromeFullVer = defaultUA.match(/Chrome\/([\d.]+)/)?.[1] || '134.0.0.0';
  const chromeMajor = chromeFullVer.split('.')[0];

  // Rewrite the OS portion of the UA to match the fingerprint profile's platform
  // so the UA and navigator.platform don't contradict each other.
  if (activeProfile?.navigator?.platform) {
    const plat = activeProfile.navigator.platform;
    if (plat === 'Win32') {
      cleanUA = `Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${chromeFullVer} Safari/537.36`;
    } else if (plat === 'MacIntel') {
      cleanUA = `Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${chromeFullVer} Safari/537.36`;
    } else if (plat === 'Linux x86_64') {
      cleanUA = `Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${chromeFullVer} Safari/537.36`;
    }
  }
  ses.setUserAgent(cleanUA);

  const platformHint = activeProfile?.navigator?.platform === 'Win32' ? 'Windows'
    : activeProfile?.navigator?.platform === 'Linux x86_64' ? 'Linux'
    : activeProfile?.navigator?.platform === 'MacIntel' ? 'macOS'
    : process.platform === 'darwin' ? 'macOS'
    : process.platform === 'win32' ? 'Windows' : 'Linux';

  // ── Sec-CH-UA: rewrite client-hint headers to hide Electron ──
  // Calling onBeforeSendHeaders replaces the previous handler (Electron behavior).
  ses.webRequest.onBeforeSendHeaders((details, callback) => {
    const headers = { ...details.requestHeaders };

    for (const key of Object.keys(headers)) {
      const lk = key.toLowerCase();
      if (lk === 'sec-ch-ua') {
        headers[key] = `"Chromium";v="${chromeMajor}", "Google Chrome";v="${chromeMajor}", "Not:A-Brand";v="24"`;
      } else if (lk === 'sec-ch-ua-full-version-list') {
        headers[key] = `"Chromium";v="${chromeFullVer}", "Google Chrome";v="${chromeFullVer}", "Not:A-Brand";v="24.0.0.0"`;
      } else if (lk === 'sec-ch-ua-platform') {
        headers[key] = `"${platformHint}"`;
      } else if (lk === 'sec-ch-ua-mobile') {
        headers[key] = '?0';
      }
    }

    callback({ requestHeaders: headers });
  });

  // ── Proxy: apply from active profile ──
  await configureProxy(ses, governance.configuration?.proxy || activeProfile?.proxy);
  governance.install(ses);
}

// ─── One-click sign-in (oya:// deep links) ───
//
// The dashboard hands out `oya://connect?key=...&server=...` so a customer can
// go from "pick Oya Browsers" to a signed-in desktop browser without copying a
// key by hand. Cookies gathered here are what the remote browsers reuse, so
// this is the step that makes an agent arrive already logged in.

/**
 * A deep link is attacker-reachable input: any page the user visits can set
 * location.href to an oya:// URL. Connecting hands the target server this
 * browser's cookies, so an unattended handler would be drive-by cookie
 * exfiltration. Two things stop that:
 *
 *   1. The link carries a single-use pairing code, never a key, and the code is
 *      exchanged over HTTPS with the server it names. A code from a hostile page
 *      only redeems against that page's own server.
 *   2. The person is asked, with the destination host spelled out and Cancel as
 *      the default. A code proves the dashboard issued the link; it does not
 *      prove the user meant to click it.
 */
async function applyDeepLink(rawUrl) {
  let url;
  try { url = new URL(rawUrl); } catch { return false; }
  if (url.protocol !== 'oya:') return false;

  const code = url.searchParams.get('code');
  const server = url.searchParams.get('server');
  if (!code || !server) return false;

  let parsed;
  try { parsed = new URL(server); } catch { return false; }
  if (!['ws:', 'wss:'].includes(parsed.protocol)) return false;
  // Plaintext ws:// is only reasonable against your own machine; anywhere else
  // it would put the key and every synced cookie on the wire in the clear.
  const local = ['localhost', '127.0.0.1', '::1', '[::1]'].includes(parsed.hostname);
  if (parsed.protocol === 'ws:' && !local) return false;

  // showMessageBox refuses a null parent, and second-instance can arrive before
  // the window exists.
  const ask = (opts) => (mainWindow ? dialog.showMessageBox(mainWindow, opts) : dialog.showMessageBox(opts));

  const { response } = await ask({
    type: 'warning',
    buttons: ['Cancel', 'Connect'],
    defaultId: 0,
    cancelId: 0,
    title: 'Connect this browser?',
    message: `Connect to ${parsed.host}?`,
    detail: 'This browser will sign in to that control plane and share its cookies and '
      + 'logged-in sessions with it, so remote browsers can act as you.\n\n'
      + 'Only continue if you started this from that dashboard. Cancel if a web page opened it.',
  });
  if (response !== 1) return false;

  const claimUrl = `${parsed.protocol === 'wss:' ? 'https' : 'http'}://${parsed.host}/api/pairing/claim`;
  let apiKey, persona;
  try {
    const res = await fetch(claimUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      redirect: 'error',
      body: JSON.stringify({ code }),
      signal: AbortSignal.timeout(15000),
    });
    const body = await res.json().catch(() => ({}));
    if (!res.ok || !body.apiKey) throw new Error(body.error || `Pairing failed (${res.status})`);
    apiKey = body.apiKey;
    persona = body.persona || 'default';
  } catch (e) {
    await ask({ type: 'error', title: 'Could not pair', message: 'Pairing failed', detail: e.message });
    return false;
  }

  // A deep link can retarget this browser at a different project (each project
  // has its own key). The durable session id is scoped to a project, so reusing
  // the previous browserId makes the new project reject the socket as a foreign
  // id — close 4003, which the client treats as fatal and never retries, so the
  // desktop sits offline until a full restart happens to mint a fresh id. Rebind
  // to a new session on any key/server change; an unchanged target keeps its id.
  if (config.apiKey !== apiKey || config.serverUrl !== parsed.href) browserId = null;
  disconnect();
  config.serverUrl = parsed.href;
  config.apiKey = apiKey;
  config.persona = persona;
  saveConfig();
  // connect() emits ws-status, which is how the renderer learns about this.
  connect();
  mainWindow?.show();
  return true;
}

// Windows and Linux deliver the link as an argv entry to a second launch;
// without the single-instance lock that launch becomes a second browser with
// its own session, and the cookies land in the wrong place.
const pendingDeepLinks = [];
if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  app.on('second-instance', (_event, argv) => {
    const link = argv.find((a) => a.startsWith('oya://'));
    if (link) applyDeepLink(link).catch((e) => console.error('[deeplink]', e.message));
    mainWindow?.show();
  });
}

// macOS delivers it as an event, which can fire before the app is ready.
app.on('open-url', (event, url) => {
  event.preventDefault();
  if (mainWindow) applyDeepLink(url).catch((e) => console.error('[deeplink]', e.message));
  else pendingDeepLinks.push(url);
});

// ─── Auto update ───
//
// Squirrel.Mac stages the new bundle and swaps it when the app quits, so an
// update never interrupts a browsing session — it is simply there next launch.
// Someone who wants it sooner gets a Restart button, wired to install-update.
//
// Cloud browsers are pinned to the snapshot they were provisioned from; one
// that upgraded itself mid-run would no longer be the build the fleet expects.

const UPDATE_CHECK_INTERVAL = 6 * 60 * 60 * 1000;
let autoUpdater = null;
let updateState = { state: 'current', version: null };

/** Single source of truth for the toolbar, so a late-loading window still learns. */
function setUpdateState(next) {
  updateState = { ...next, current: app.getVersion() };
  sendToRenderer('update-status', updateState);
}

function startAutoUpdate() {
  if (!app.isPackaged || process.env.OYA_DOCKER === 'true') {
    setUpdateState({ state: 'unsupported', version: null });
    return;
  }
  // Required lazily so dev runs and cloud browsers never load it at all.
  ({ autoUpdater } = require('electron-updater'));
  autoUpdater.autoDownload = true;
  autoUpdater.autoInstallOnAppQuit = true;

  autoUpdater.on('checking-for-update', () => setUpdateState({ state: 'checking', version: null }));
  autoUpdater.on('update-not-available', () => setUpdateState({ state: 'current', version: null }));

  autoUpdater.on('update-available', (info) => {
    console.log('[update] available:', info.version);
    setUpdateState({ state: 'available', version: info.version });
    // The window is often not the thing being looked at, so say it once at the
    // OS level too. Only on the transition — never on the periodic re-checks.
    if (Notification.isSupported() && notifiedVersion !== info.version) {
      notifiedVersion = info.version;
      new Notification({
        title: `Oya Browser ${info.version} is available`,
        body: `You are on ${app.getVersion()}. It installs when you quit, or restart now from the toolbar.`,
        silent: true,
      }).show();
    }
  });

  autoUpdater.on('download-progress', ({ percent, version }) => {
    setUpdateState({ state: 'downloading', version: version || updateState.version, percent: Math.round(percent) });
  });

  autoUpdater.on('update-downloaded', (info) => {
    console.log('[update] ready, applies on quit:', info.version);
    setUpdateState({ state: 'ready', version: info.version });
  });

  autoUpdater.on('error', (err) => {
    // A missed check is not worth interrupting anyone over; the next one retries.
    console.log('[update] check failed:', err?.message || err);
    setUpdateState({ state: 'error', version: null, message: err?.message || String(err) });
  });

  const check = () => autoUpdater.checkForUpdates().catch(() => {});
  setTimeout(check, 10000).unref?.();          // let the first window settle
  setInterval(check, UPDATE_CHECK_INTERVAL).unref?.();
}

let notifiedVersion = null;

ipcMain.handle('check-for-updates', async () => {
  if (!autoUpdater) return updateState;
  try {
    await autoUpdater.checkForUpdates();
  } catch (e) {
    setUpdateState({ state: 'error', version: null, message: e?.message || String(e) });
  }
  return updateState;
});

ipcMain.handle('get-update-status', () => updateState);

ipcMain.handle('install-update', () => {
  if (!autoUpdater || updateState.state !== 'ready') return false;
  // Nothing else gets to run after this — it relaunches the app.
  setImmediate(() => autoUpdater.quitAndInstall());
  return true;
});

ipcMain.handle('get-version', () => app.getVersion());

// ─── App Lifecycle ───

function installApplicationMenu() {
  const humanAction = run => () => { if (desktopControl.snapshot().interactive) run(); };
  app.setAboutPanelOptions({ applicationName: 'Oya Browser', applicationVersion: app.getVersion() });
  Menu.setApplicationMenu(Menu.buildFromTemplate([
    ...(process.platform === 'darwin' ? [{ label: 'Oya Browser', submenu: [
      { role: 'about', label: 'About Oya Browser' }, { type: 'separator' },
      { role: 'services' }, { type: 'separator' },
      { role: 'hide', label: 'Hide Oya Browser' }, { role: 'hideOthers' }, { role: 'unhide' },
      { type: 'separator' }, { role: 'quit', label: 'Quit Oya Browser' },
    ] }] : []),
    { label: 'File', submenu: [
      { id: 'browser-new-tab', label: 'New Tab', accelerator: 'CmdOrCtrl+T', click: humanAction(() => createTab('https://google.com', true)) },
      { id: 'browser-close-tab', label: 'Close Tab', accelerator: 'CmdOrCtrl+W', click: humanAction(() => closeTab(activeTabId)) },
      ...(process.platform === 'darwin' ? [] : [{ role: 'quit', label: 'Quit Oya Browser' }]),
    ] },
    { role: 'editMenu' },
    { label: 'View', submenu: [
      { id: 'browser-reload', label: 'Reload Page', accelerator: 'CmdOrCtrl+R', click: humanAction(reloadActivePage) },
      { type: 'separator' }, { role: 'resetZoom' }, { role: 'zoomIn' }, { role: 'zoomOut' },
      { type: 'separator' }, { role: 'togglefullscreen' },
    ] },
    { role: 'windowMenu' },
  ]));
}
app.whenReady().then(async () => {
  installApplicationMenu();
  loadConfig();

  // Registering in dev needs the interpreter and script path, or the OS
  // registers the wrong executable.
  if (process.defaultApp && process.argv.length >= 2) {
    app.setAsDefaultProtocolClient('oya', process.execPath, [path.resolve(process.argv[1])]);
  } else {
    app.setAsDefaultProtocolClient('oya');
  }

  // Initialize profile store and load active profile
  profileStore = new ProfileStore(app.getPath('userData'));
  const activeId = config.activeProfileId || profileStore.getActiveId();
  if (activeId) {
    activeProfile = profileStore.get(activeId);
  }

  await setupBrowserSession();
  workspace = new Workspace({
    store: new DraftStore(path.join(app.getPath('userData'), 'workflow-drafts'), safeStorage),
    runStore: new DraftStore(path.join(app.getPath('userData'), 'workflow-runs'), safeStorage),
    notify: state => sendToRenderer('workspace-state', state),
    runner: (draft, options, event) => require('./scripts/validation.cjs').validate({ draft, options, event, app, utilityProcess, control: desktopControl, tabs: () => tabs, createTab: url => { if (!browsingMode) { enterBrowsingMode(url); return activeTabId; } return createTab(url, true); }, closeTab, cdpPort: CDP_PORT ? CDP_PORT + 1 : 0 }),
  });
  recordedSteps = workspace.draft.steps; recordedSecrets = new Set(workspace.draft.secrets);
  captureSignature = JSON.stringify([false, recordedSteps, [...recordedSecrets]]);
  createWindow();
  if (CDP_PORT) require('./cdp-front-door').start({
    port: CDP_PORT, upstream: CDP_PORT + 1,
    host: process.env.OYA_REMOTE_DEBUGGING_HOST || (process.env.OYA_DOCKER === 'true' ? '0.0.0.0' : '127.0.0.1'),
    tabs: () => tabs,
    // Outside browsing mode a tab is never laid out, and a page with a 0x0
    // viewport is both broken and an obvious bot.
    createTab: (url) => {
      if (browsingMode) return createTab(url, true);
      enterBrowsingMode(url);
      return activeTabId;
    },
    closeTab,
    beginCommand: () => desktopControl.beginLocalCommand(),
    clientChanged: delta => desktopControl.localClient(delta),
    relayToken: CDP_RELAY_TOKEN,
  });
  startCookieChangeListener();
  if (config.apiKey || process.env.OYA_AUTO_CONNECT === 'true') connect();

  startAutoUpdate();

  const queued = pendingDeepLinks.splice(0)
    .concat(process.argv.filter((a) => a.startsWith('oya://')));
  for (const link of queued) await applyDeepLink(link).catch((e) => console.error('[deeplink]', e.message));
});

app.on('window-all-closed', () => { disconnect(); app.quit(); });

function createWindow() {
  devPanelWidth = Number(config.ui?.panelWidth) || 360;
  mainWindow = new BrowserWindow({
    width: 1280, height: 860, minWidth: 600, minHeight: 400,
    icon: path.join(__dirname, 'build', process.platform === 'darwin' ? 'icon.icns' : 'icon_1024.png'),
    titleBarStyle: process.platform === 'darwin' ? 'hiddenInset' : 'default',
    trafficLightPosition: process.platform === 'darwin' ? { x: 12, y: 12 } : undefined,
    backgroundColor: shellBackground(),
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true, nodeIntegration: false,
    },
  });
  mainWindow.loadFile(path.join(__dirname, 'renderer/index.html')).then(prepareControlShield);
  installShellShortcuts(mainWindow.webContents);
  mainWindow.webContents.on('did-finish-load', layoutActiveTab);
  nativeTheme.on('updated', () => {
    if (!mainWindow || mainWindow.isDestroyed()) return;
    mainWindow.setBackgroundColor(shellBackground());
    sendToRenderer('shell-appearance', nativeTheme.shouldUseDarkColors);
  });
  if (process.env.OYA_DOCKER === 'true') mainWindow.maximize();
  mainWindow.on('resize', layoutActiveTab);
}

function shellBackground() {
  const dark = config.ui?.theme === 'dark' || (config.ui?.theme !== 'light' && nativeTheme.shouldUseDarkColors);
  return dark ? '#1b1e1c' : '#f5f5f2';
}

function installShellShortcuts(contents) {
  contents.on('before-input-event', (event, input) => {
    if (contents !== mainWindow?.webContents && !desktopControl.snapshot().interactive) event.preventDefault();
    if (input.type !== 'keyDown' || !(process.platform === 'darwin' ? input.meta : input.control) || input.alt) return;
    const key = input.code === 'BracketRight' ? ']' : input.code === 'BracketLeft' ? '[' : input.key.toLowerCase();
    const command = input.shift ? ({ r: 'record', d: 'tools', ']': 'next-tab', '[': 'previous-tab' })[key]
      : ({ l: 'address', k: 'commands', t: 'new-tab', w: 'close-tab' })[key];
    if (!command) return;
    event.preventDefault();
    if (command === 'new-tab') { if (desktopControl.snapshot().interactive) { createTab('https://google.com', true); recordNavigation('https://google.com'); } }
    else if (command === 'close-tab') { if (desktopControl.snapshot().interactive) closeTab(activeTabId); }
    else if (command.endsWith('-tab')) {
      const index = tabs.findIndex(tab => tab.id === activeTabId);
      const target = tabs[(index + (command === 'next-tab' ? 1 : tabs.length - 1)) % tabs.length];
      if (target) activateTab(target.id);
    } else {
      mainWindow.webContents.focus();
      sendToRenderer('shell-command', command);
    }
  });
}

// ─── Tab Management ───

function createTab(url, activate = true) {
  const id = nextTabId++;
  const view = new BrowserView({
    webPreferences: {
      contextIsolation: true, sandbox: true,
      partition: getPartitionName(),
    },
  });

  const tab = { id, view, title: 'New Tab', url: url || '' };
  tabs.push(tab);
  installShellShortcuts(view.webContents);
  view.webContents.on('focus', () => { if (!desktopControl.snapshot().interactive) mainWindow?.webContents.focus(); });
  view.webContents.on('did-start-loading', () => { tab.loadError = null; sendTabList(); });
  view.webContents.on('did-stop-loading', sendTabList);
  view.webContents.on('did-fail-load', (_event, code, description, _url, mainFrame) => {
    if (!mainFrame || code === -3) return;
    tab.navigationPending = false; tab.loadError = `Page could not load: ${description}. Try Reload.`; sendTabList();
  });
  view.webContents.on('render-process-gone', (_event, details) => {
    tab.navigationPending = false; tab.loadError = `Page renderer stopped (${details.reason}). Reload to recover.`; sendTabList();
  });

  // Attach CDP debugger and auto-inject scripts into every new document.
  // A view has no renderer until its first navigation, and CDP's Page domain
  // does not answer before there is one — so awaiting setup before loadURL was
  // a deadlock that only the timeout broke, and every tab's first page loaded
  // unprotected. about:blank starts the renderer without a network request.
  // The race does not cancel the losing sleep, so it checks whether setup
  // finished — otherwise every healthy tab reported itself unprotected.
  let setupDone = false;
  const tabReady = Promise.race([
    view.webContents.loadURL('about:blank').catch(() => {}).then(() => setupTabCDP(view)).finally(() => { setupDone = true; }),
    sleep(CDP_SETUP_TIMEOUT).then(() => {
      if (!setupDone) console.error(`[anonymity] CDP setup unfinished after ${CDP_SETUP_TIMEOUT}ms — loading anyway, this tab may be UNPROTECTED`);
    }),
  ]);

  view.webContents.on('did-finish-load', () => {
    injectScripts(view);
    // Make view-source pages readable (force light theme)
    const currentUrl = view.webContents.getURL();
    if (currentUrl.startsWith('view-source:')) {
      view.webContents.executeJavaScript(`
        document.documentElement.style.cssText = 'background:#fff!important;color:#000!important;color-scheme:light!important';
        document.body.style.cssText = 'background:#fff!important;color:#000!important';
        const s = document.createElement('style');
        s.textContent = '*, *::before, *::after { color-scheme: light !important; } body, html, .line-content, .line-number, td, tr, table { background-color: #fff !important; color: #000 !important; } a { color: #00e !important; }';
        document.head.appendChild(s);
      `, true).catch(() => {});
    }
  });

  const updateUrl = (e, u) => {
    tab.url = u;
    if (tab.id === activeTabId) sendToRenderer('url-changed', u);
    sendTabList();
  };
  view.webContents.on('did-navigate', updateUrl);
  view.webContents.on('did-navigate-in-page', updateUrl);
  // New tabs join an active recording before the user can interact with them.
  tabReady.then(() => { if (recording) return armRecordingView(view); }).catch((err) => console.error('[recording]', err));
  view.webContents.on('page-title-updated', (e, title) => {
    tab.title = title;
    if (tab.id === activeTabId) sendToRenderer('title-changed', title);
    sendTabList();
  });

  // target="_blank" / window.open → new tab
  // But allow OAuth popups (Google, GitHub, etc.) to work natively
  view.webContents.setWindowOpenHandler(({ url, features }) => {
    const isAuthPopup = url.includes('accounts.google.com') ||
      url.includes('github.com/login/oauth') ||
      url.includes('login.microsoftonline.com') ||
      url.includes('appleid.apple.com') ||
      url.includes('x.com') ||
      url.includes('twitter.com') ||
      url.includes('api.twitter.com') ||
      url.includes('arkoselabs.com') ||
      (features && features.includes('popup'));

    if (isAuthPopup) {
      return {
        action: 'allow',
        overrideBrowserWindowOptions: {
          width: 500, height: 700,
          webPreferences: { partition: getPartitionName() },
        },
      };
    }

    createTab(url, true);
    return { action: 'deny' };
  });

  // Configure child windows created by allowed popups (OAuth, 2FA, etc.)
  //
  // These must be protected BEFORE the popup's own scripts run. Injecting on
  // did-finish-load meant the document had already executed — and the popup
  // allowlist includes bot-detection vendors, which therefore read a completely
  // unspoofed browser and only saw the overrides afterwards.
  view.webContents.on('did-create-window', (childWindow) => {
    controlPopups.add(childWindow);
    childWindow.setEnabled(desktopControl.snapshot().interactive);
    childWindow.once('closed', () => controlPopups.delete(childWindow));
    try {
      const dbg = childWindow.webContents.debugger;
      if (!dbg.isAttached()) dbg.attach(CDP_VERSION);
      applyPersona(dbg, (what, e) => console.error(`[anonymity] popup ${what} failed — popup is NOT protected:`, e?.message || e));
      dbg.sendCommand('Page.enable').catch(() => {});

      // A sign-in popup is where the session actually gets written, so it needs
      // the same localStorage transport a tab gets. Without this the cookies
      // synced but the token half of a login stayed on this machine.
      if (loginState) {
        loginState.attach(
          (method, params = {}) => dbg.sendCommand(method, params),
          (method, fn) => dbg.on('message', (_event, event, params) => { if (event === method) fn(params); }),
        ).catch((e) => console.error('[oya] popup login state not synced:', e.message));
      }
    } catch (e) {
      console.error('[anonymity] popup debugger attach failed — popup is NOT protected:', e.message);
    }
  });

  // Right-click context menu with DevTools, View Source, Inspect
  view.webContents.on('context-menu', (e, params) => {
    const menu = Menu.buildFromTemplate([
      ...(params.linkURL ? [
        { label: 'Open Link in New Tab', click: () => createTab(params.linkURL, true) },
        { type: 'separator' },
      ] : []),
      { label: 'Back', enabled: view.webContents.navigationHistory.canGoBack(), click: () => view.webContents.goBack() },
      { label: 'Forward', enabled: view.webContents.navigationHistory.canGoForward(), click: () => view.webContents.goForward() },
      { label: 'Reload', click: () => view.webContents.reload() },
      { type: 'separator' },
      { label: 'Copy', role: 'copy', enabled: params.editFlags.canCopy },
      { label: 'Paste', role: 'paste', enabled: params.editFlags.canPaste },
      { label: 'Select All', role: 'selectAll' },
      { type: 'separator' },
      { label: 'View Page Source', click: async () => {
        try {
          await injectScripts(view);
          const html = await worldEval(view, 'document.documentElement.outerHTML');
          let markdown = '';
          try {
            const result = await worldEval(view,
              '(typeof analyzePage === "function") ? analyzePage({}) : null');
            if (result?.ok) markdown = result.data.markdown || '';
          } catch {}
          if (!devPanelOpen) {
            devPanelOpen = true;
            layoutActiveTab();
            sendToRenderer('dev-panel-state', devPanelOpen);
          }
          sendToRenderer('view-source', { html, markdown, url: view.webContents.getURL() });
        } catch (e) {
          sendToRenderer('view-source', { html: '', markdown: '', error: e.message });
        }
      }},
      { label: 'Inspect Element', click: async () => {
        try {
          await injectScripts(view);
          const result = await worldEval(view, `
            (function() {
              const el = document.elementFromPoint(${params.x}, ${params.y});
              if (!el) return { ok: false, error: 'No element at coordinates' };
              // Walk up to find nearest element with data-ac-id, or use the element itself
              let target = el;
              while (target && !target.getAttribute('data-ac-id') && target !== document.body) {
                target = target.parentElement;
              }
              // Run analyzePage scoped to this element's parent section
              if (typeof analyzePage === 'function') {
                // Find a reasonable scope — the element's closest section/article/main or its parent
                let scope = el.closest('section, article, main, [role="main"], [role="dialog"], form, nav, aside') || el.parentElement || el;
                // Generate a unique temporary selector
                const tmpId = '__oya_inspect_' + Date.now();
                scope.setAttribute('data-oya-inspect', tmpId);
                const result = analyzePage({ selector: '[data-oya-inspect="' + tmpId + '"]', highlight: true });
                scope.removeAttribute('data-oya-inspect');
                return result;
              }
              return { ok: false, error: 'Analyzer not loaded' };
            })()
          `, true);
          // Open dev panel and show result in source pane
          if (!devPanelOpen) {
            devPanelOpen = true;
            layoutActiveTab();
            sendToRenderer('dev-panel-state', devPanelOpen);
          }
          sendToRenderer('inspect-result', result);
        } catch (e) {
          sendToRenderer('inspect-result', { ok: false, error: e.message });
        }
      }},
      { label: 'Open DevTools', click: () => view.webContents.openDevTools({ mode: 'detach' }) },
    ]);
    menu.popup({ window: mainWindow });
  });

  tab.setup = tabReady;
  tab.ready = Promise.resolve(tabReady).then(() => url && !tab.navigationRequest ? view.webContents.loadURL(url) : undefined);
  tab.ready.catch((e) => console.error('[tab] Could not open page:', e.message));
  if (activate) activateTab(id);
  sendTabList();
  return id;
}

/**
 * The persona on one webContents, applied as the server's CDP driver applies
 * it (anonymity/apply.js): emulation, the injection, and the tab's workers and
 * cross-site iframes. The screen stays the window's own; the session already
 * carries the UA, workers included.
 */
function applyPersona(dbg, fail) {
  if (!activeProfile) {
    return dbg.sendCommand('Page.addScriptToEvaluateOnNewDocument', { source: buildInjectionScript(null) })
      .catch((e) => fail('stealth injection', e));
  }
  return createPersonaApplier({
    send: (method, params, sessionId) => dbg.sendCommand(method, params, sessionId),
    on: (event, fn) => dbg.on('message', (_e, method, params, sessionId) => { if (method === event) fn(params, sessionId); }),
    profile: activeProfile,
    screen: false,
    onError: fail,
  }).page();
}

async function setupTabCDP(view) {
  const fail = (what, err) => {
    // A silent failure here means a tab that loads with no fingerprint and no
    // stealth, and at fleet scale you cannot tell which browsers are naked.
    console.error('[anonymity] ' + what + ' failed — this tab is NOT protected:', err?.message || err);
  };

  try {
    if (!cdpAttach(view)) return fail('debugger attach', new Error('view destroyed'));

    // Main world: only what the page itself must see, as one script in one
    // scope so the toString mask covers the fingerprint patches too. The
    // analyzer is loaded separately into an isolated world by ensureWorld().
    if (view.oyaConfigured) return;
    view.oyaConfigured = true;
    await applyPersona(view.webContents.debugger, fail);
    view.webContents.debugger.sendCommand('Page.enable').catch((e) => fail('Page.enable', e));
    if (loginState) await loginState.attach(
      (method, params = {}) => cdp(view, method, params),
      (method, fn) => view.webContents.debugger.on('message', (_event, event, params) => { if (event === method) fn(params); }),
    );

    // A fresh document means a fresh isolated world; rebuild it eagerly so the
    // first command after a navigation does not pay for it.
    view.webContents.on('did-finish-load', () => {
      ensureWorld(view, { force: true }).catch((e) => fail('isolated world', e));
    });
  } catch (e) {
    fail('CDP setup', e);
  }
}

function activateTab(id) {
  const tab = tabs.find(t => t.id === id);
  if (!tab) return;
  if (activeTabId === id) return;
  activeTabId = id;
  if (!shellOverlays.size) mainWindow.setBrowserView(tab.view);
  layoutActiveTab();
  sendToRenderer('url-changed', tab.url);
  sendToRenderer('title-changed', tab.title);
  sendTabList();
}

function closeTab(id, { keepOne = true } = {}) {
  const idx = tabs.findIndex(t => t.id === id);
  if (idx === -1) return;
  const tab = tabs[idx];
  const wasActive = tab.id === activeTabId;

  try { mainWindow.removeBrowserView(tab.view); } catch {}
  tabs.splice(idx, 1);

  // Detach debugger before destroying
  try {
    if (tab.view.webContents.debugger.isAttached()) tab.view.webContents.debugger.detach();
  } catch {}
  try {
    if (!tab.view.webContents.isDestroyed()) tab.view.webContents.destroy();
  } catch {}

  if (tabs.length === 0) {
    activeTabId = null;
    // Only the user-facing close paths keep a window's worth of browser alive.
    // A bulk close (profile switch) wants the list actually empty — recreating
    // here made `while (tabs.length)` loop forever, spawning a renderer per turn.
    if (keepOne) createTab('https://google.com', true);
    else sendTabList();
  } else if (wasActive) {
    activeTabId = null;
    activateTab(tabs[Math.min(idx, tabs.length - 1)].id);
  } else {
    sendTabList();
  }
}

function getActiveView() {
  return tabs.find(t => t.id === activeTabId)?.view || null;
}

function sendTabList() {
  sendToRenderer('tabs-updated', tabs.map(t => ({
    id: t.id, title: t.title, url: t.url, active: t.id === activeTabId,
    loading: !!t.navigationPending || t.view.webContents.isLoading(), loadError: t.loadError || null,
    canGoBack: t.view.webContents.navigationHistory.canGoBack(),
    canGoForward: t.view.webContents.navigationHistory.canGoForward(),
  })));
}

let panelMotionTimer;
let panelProgress = 0;
let panelSaveTimer;
let finishingQuit = false;
app.on('before-quit', event => {
  if (recording && !finishingQuit) {
    event.preventDefault(); finishingQuit = true;
    queueRecording(stopRecording).finally(() => app.quit()); return;
  }
  if (workspace?.busy()) { workspace.receive({ type: 'finished', status: 'interrupted', error: 'Oya closed during validation. Check the website before retrying.' }); workspace.session?.dispose(); }
  clearInterval(panelMotionTimer);
  if (panelSaveTimer) { clearTimeout(panelSaveTimer); saveConfig(); }
});
function layoutActiveTab(progress) {
  if (typeof progress !== 'number') {
    clearInterval(panelMotionTimer);
    progress = devPanelOpen ? 1 : 0;
  }
  panelProgress = progress;
  const view = getActiveView();
  if (!mainWindow) return;
  const bounds = mainWindow.getContentBounds();
  const layout = shellLayout(bounds.width, bounds.height, progress, devPanelWidth);
  sendToRenderer('shell-layout', layout);
  if (view && browsingMode) view.setBounds(layout.page);
  syncControlShield();
}

function enterBrowsingMode(url) {
  if (browsingMode) return;
  browsingMode = true;
  createTab(url || 'https://google.com', true);
  sendToRenderer('mode-changed', 'browsing');
}

// ─── Script Injection (fallback — CDP auto-inject is primary) ───

/** Ensure the analyzer is loaded in this view's isolated world. */
async function injectScripts(view) {
  if (!view) view = getActiveView();
  if (!view) return;
  try {
    await ensureWorld(view);
  } catch (e) {
    console.error('[anonymity] isolated world unavailable, analyzer not loaded:', e.message);
  }
}

// ─── Recording: a person demonstrates the task, the server keeps it as a playbook ───
//
// The page buffers what the user does (scripts/analyzer.js) in the same step shape the
// agent produces, so a recording gets replay, healing and the Playwright export for
// free from server/src/playbook.js. Nothing here interprets the steps.

let recording = false;
let recordingOrigin = 'desktop';
let recordingCutoff = Infinity;
let recordedSteps = [];
let recordedSecrets = new Set();
let drainTimer = null;
const recordingChannels = new Map();
const recordedIds = new Set();
const pausedUrls = new Map();   // tab id -> the page recording was paused on

let recordingTask = Promise.resolve();
function queueRecording(work) {
  const next = recordingTask.then(work);
  recordingTask = next.catch(() => {});
  return next;
}

/** The HTTP origin behind the control socket. */
function serverHttpBase() {
  return (config.serverUrl || '').replace(/^wss/, 'https').replace(/^ws/, 'http').replace(/\/ws\/?$/, '');
}

function pushRecordedStep(step) {
  if (!recording || (step.t || Date.now()) > recordingCutoff) return;
  if (recordedSteps.length >= 500) { recordedSteps[499].captureIssue = 'The 500-step capture limit was reached. Later actions were not recorded. Split this workflow and review its ending.'; queueRecording(stopRecording); return; }
  if (step.id && recordedIds.has(step.id)) return;
  if (step.id) recordedIds.add(step.id);
  recordedSteps.push(normalizeStep({ t: Date.now(), tab: recordingTab(activeTabId), ...step }));
}

/**
 * Only a URL the person asked for — the address bar, a new tab. Where a click or a
 * form submission lands is already the click's step, and a goto over it replays past
 * whatever that click set up (and pins a one-off session URL into the playbook).
 */
function recordNavigation(url) {
  if (!recording || !/^https?:\/\//i.test(url || '')) return;
  const last = recordedSteps[recordedSteps.length - 1];
  if (last && last.action === 'navigate' && last.url === url && last.tab === recordingTab(activeTabId)) return;
  pushRecordedStep({ action: 'navigate', url });
}

/** Collect what one page buffered. Steps carry their own timestamps; the merge sorts by them. */
async function drainView(view, final = false) {
  if (!view) return;
  await recordingChannels.get(view)?.drain(final);
  recordedSteps.sort((a, b) => a.t - b.t);
}

async function armRecordingView(view) {
  if (recordingChannels.has(view)) return recordingChannels.get(view).ready;
  const startingUrl = view.webContents.getURL();
  const channel = new RecordingChannel({
    send: (method, params) => cdp(view, method, params),
    on: (method, fn) => {
      const dbg = cdpAttach(view);
      const listener = (_event, name, params) => { if (name === method) fn(params); };
      dbg.on('message', listener);
      return () => dbg.off('message', listener);
    },
    disableRuntimeOnStop: true,
    worldName: ISOLATED_WORLD,
    analyzer: analyzerScript.replace('__OYA_ATTR__', 'data-' + require('crypto').randomBytes(4).toString('hex')).replace('__OYA_RECORD__', 'false'),
    receive: (out) => {
      if (!recording) return;
      for (const name of out.secrets || []) recordedSecrets.add(name);
      const owner = tabs.find(t => t.view === view), tabName = recordingTab(owner?.id);
      if (out.steps?.length && !recordedSteps.some(step => step.tab === tabName) && /^https?:\/\//i.test(startingUrl)) pushRecordedStep({ action: 'navigate', url: startingUrl, tab: tabName, t: (out.steps[0].t || Date.now()) - 1 });
      for (const step of out.steps || []) { const tab = tabs.find(t => t.view === view); pushRecordedStep({ ...step, tab: recordingTab(tab?.id) }); }
    },
  });
  recordingChannels.set(view, channel);
  channel.ready = channel.start();
  try { await channel.ready; }
  catch (err) { recordingChannels.delete(view); throw err; }
}

async function drainAll(final = false) {
  for (const tab of tabs) await drainView(tab.view, final);
  emitRecording();
}

async function startRecording(resume = false, origin = 'desktop') {
  if (recording) return { recording: true, steps: recordedSteps };
  if (workspace?.busy()) throw new Error('Stop validation before recording');
  if (!resume && workspace) workspace.edit({ type: 'new' });
  recording = true; recordingOrigin = origin; recordingCutoff = Infinity;
  recordedSteps = resume && workspace ? structuredClone(workspace.draft.steps) : [];
  recordedSecrets = new Set(resume && workspace ? workspace.draft.secrets : []);
  recordedIds.clear(); for (const step of recordedSteps) recordedIds.add(step.id);
  if (!resume) { recordingTabMap.clear(); pausedUrls.clear(); }
  if (!recordingTabMap.size) recordingTabMap.set(activeTabId, resume ? recordedSteps.at(-1)?.tab || 'main' : 'main');
  const view = getActiveView();
  const url = view?.webContents.getURL();
  // Replay has to start where the person started, the way an ask() run does — and when
  // they browsed somewhere else while recording was paused, replay has to follow them
  // there, or every step after the resume runs against the page the pause left behind.
  if (/^https?:\/\//i.test(url || '')) {
    if (!resume) pushRecordedStep({ action: 'navigate', url, start: true });
    else if (pausedUrls.has(activeTabId) && pausedUrls.get(activeTabId) !== url) pushRecordedStep({ action: 'navigate', url });
  }
  try {
    for (const tab of tabs) await armRecordingView(tab.view);
  } catch (err) {
    await stopRecording();
    throw err;
  }
  drainTimer = setInterval(emitRecording, 400);
  emitRecording();
  return { recording: true, steps: recordedSteps };
}

async function stopRecording() {
  if (!recording) return { recording: false, steps: recordedSteps };
  clearInterval(drainTimer);
  drainTimer = null;
  for (const [view, channel] of recordingChannels) {
    await channel.ready.catch(() => {});
    await channel.stop().catch((err) => { if (!view.webContents.isDestroyed()) throw err; });
  }
  recordingChannels.clear();
  await drainAll(true);
  recording = false;
  for (const tab of tabs) { try { pausedUrls.set(tab.id, tab.view.webContents.getURL()); } catch {} }
  emitRecording();
  return { recording: false, steps: recordedSteps };
}

// ─── IPC ───

ipcMain.handle('navigate', async (e, url) => {
  requireHumanControl();
  if (!browsingMode) { enterBrowsingMode(url); return; }
  const view = getActiveView();
  if (!view) return;
  const tab = tabs.find(tab => tab.view === view);
  const request = tab.navigationRequest = (tab.navigationRequest || 0) + 1;
  tab.navigationPending = true; tab.loadError = null; sendTabList();
  url = String(url).trim();
  if (!/^https?:\/\//i.test(url)) url = 'https://' + url;
  recordNavigation(url);
  await Promise.all([pullCookiesFor(url), tab.setup]);
  if (view.webContents.isDestroyed() || tab.navigationRequest !== request) return;
  if (!desktopControl.snapshot().interactive) { tab.navigationPending = false; sendTabList(); return; }
  tab.navigationPending = false;
  view.webContents.loadURL(url).catch(() => {});
  sendTabList();
});

ipcMain.handle('go-back', () => { requireHumanControl(); getActiveView()?.webContents.goBack(); });
ipcMain.handle('go-forward', () => { requireHumanControl(); getActiveView()?.webContents.goForward(); });
function reloadActivePage() {
  requireHumanControl();
  const tab = tabs.find(tab => tab.id === activeTabId);
  if (!tab) return;
  if (tab.navigationPending || tab.view.webContents.isLoading()) {
    tab.navigationRequest = (tab.navigationRequest || 0) + 1;
    tab.navigationPending = false; tab.view.webContents.stop(); sendTabList();
  } else { tab.loadError = null; tab.view.webContents.reload(); }
}
ipcMain.handle('reload', reloadActivePage);
ipcMain.handle('get-control-state', () => desktopControl.snapshot());
ipcMain.handle('change-control', async (_event, action) => {
  try { return { state: await desktopControl.change(action) }; }
  catch (error) { return { error: error.message, state: desktopControl.snapshot() }; }
});
ipcMain.handle('get-config', () => config);

ipcMain.handle('save-config', (e, newConfig) => {
  config = { ...config, ...newConfig };
  saveConfig();
  disconnect();
  connect();
  return true;
});

ipcMain.handle('get-status', () => ({
  connected: wsReady, browserId,
  url: getActiveView()?.webContents.getURL() || '',
}));

ipcMain.handle('enter-browsing', () => enterBrowsingMode('https://google.com'));
ipcMain.handle('new-tab', (e, url) => { requireHumanControl(); const id = createTab(url || 'https://google.com', true); recordNavigation(url || 'https://google.com'); return id; });
ipcMain.handle('close-tab', (e, id) => { requireHumanControl(); closeTab(id); });
ipcMain.handle('activate-tab', (e, id) => activateTab(id));

function requireShell(event) {
  if (event.sender !== mainWindow?.webContents || event.senderFrame !== mainWindow.webContents.mainFrame) throw new Error('Only the Oya workspace can use this command');
}
ipcMain.handle('workspace', async (event, command = {}) => {
  requireShell(event);
  if (!workspace) throw new Error('Workspace is starting');
  if (command.type === 'get') return workspace.snapshot();
  if (command.type === 'resume-recording') { requireHumanControl(); await queueRecording(() => startRecording(true)); return workspace.snapshot(); }
  if (command.type === 'pick') {
    requireHumanControl();
    if (workspace.busy() || recording) throw new Error('Pause recording and stop validation before picking a target');
    const draftId = workspace.draft.id;
    const view = getActiveView(); if (!view) throw new Error('Open a page first');
    const candidates = await require('./scripts/target-picker.cjs').pickTarget(view);
    if (workspace.draft.id !== draftId) throw new Error('Draft changed during target selection');
    const state = workspace.edit({ type: 'update', id: command.id, patch: { candidates, captureIssue: undefined } });
    recordedSteps = structuredClone(workspace.draft.steps); return state;
  }
  if (command.type === 'validate') {
    const answer = await dialog.showMessageBox(mainWindow, { type: 'question', title: 'Validate workflow', message: 'Run on the real website?', detail: 'Oya opens a fresh tab using your current login. This can submit forms, send messages, upload files, or change data. Steps run exactly as shown in the exported Playwright module.', buttons: ['Cancel', 'Run workflow'], defaultId: 0, cancelId: 0 });
    if (answer.response !== 1) return workspace.snapshot();
    return workspace.start(command);
  }
  if (command.type === 'control') {
    if (['resume', 'step'].includes(command.command) && desktopControl.snapshot().mode === 'human' && desktopControl.snapshot().mine) await desktopControl.change('return');
    return workspace.control(command.command);
  }
  if (command.type === 'support') {
    const report = workspace.support();
    const result = await dialog.showSaveDialog(mainWindow, { defaultPath: 'oya-diagnostics.json', filters: [{ name: 'JSON diagnostics', extensions: ['json'] }] });
    if (!result.canceled && result.filePath) fs.writeFileSync(result.filePath, JSON.stringify(report, null, 2), { mode: 0o600 });
    return workspace.snapshot();
  }
  const state = workspace.edit(command);
  recordedSteps = structuredClone(workspace.draft.steps); recordedSecrets = new Set(workspace.draft.secrets);
  captureSignature = JSON.stringify([recording, recordedSteps, [...recordedSecrets]]);
  return state;
});

ipcMain.handle('start-recording', () => { requireHumanControl(); return queueRecording(startRecording); });
ipcMain.handle('stop-recording', () => queueRecording(stopRecording));
ipcMain.handle('clear-recording', () => queueRecording(async () => {
  for (const channel of recordingChannels.values()) await channel.clear();
  recordedSteps = []; recordedSecrets = new Set(); recordedIds.clear();
  const url = getActiveView()?.webContents.getURL();
  if (recording && /^https?:\/\//i.test(url || '')) pushRecordedStep({ action: 'navigate', url, start: true });
  emitRecording();
  return { recording, steps: recordedSteps };
}));

/** Hand the recording to the server, which saves it as a playbook and returns its Playwright code. */
ipcMain.handle('save-recording', (e, name, description) => queueRecording(async () => {
  if (!wsReady || !browserId) return { error: 'Not connected to server' };
  if (recording) await stopRecording();
  if (!recordedSteps.length) return { error: 'Nothing recorded yet' };
  const publishingId = workspace?.draft.id, publishingRevision = workspace?.draft.revision;
  try {
    const res = await fetch(`${serverHttpBase()}/api/browsers/${browserId}/playbooks`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${config.apiKey}` },
      body: JSON.stringify({
        schemaVersion: 2, variables: workspace?.draft.variables || {},
        name,
        prompt: description,
        steps: recordedSteps.map(({ t, ...step }) => step),
        secrets: [...recordedSecrets],
      }),
    });
    const body = await res.json().catch(() => ({ error: `Server returned ${res.status}` }));
    if (res.ok && workspace?.draft.id === publishingId && workspace.draft.revision === publishingRevision) { workspace.draft.publishedAt = Date.now(); workspace.persist(); }
    return body;
  } catch (err) {
    return { error: err.message };
  }
}));

ipcMain.handle('backdrop-ready', (_event, token) => { backdropWaiters.get(token)?.(); });
ipcMain.handle('show-overlay', async (_e, name = 'legacy') => {
  if (!['legacy', 'shell'].includes(name)) return;
  const view = getActiveView();
  if (!shellOverlays.size && view && browsingMode) {
    try {
      const screenshot = await view.webContents.capturePage();
      const token = crypto.randomUUID();
      await new Promise(resolve => {
        const timer = setTimeout(() => { backdropWaiters.delete(token); resolve(); }, 300);
        backdropWaiters.set(token, () => { clearTimeout(timer); backdropWaiters.delete(token); resolve(); });
        sendToRenderer('page-backdrop', { token, image: screenshot.toDataURL(), bounds: view.getBounds() });
      });
    } catch { /* A crashed page has no frame to preserve; the reload status remains visible. */ }
  }
  shellOverlays.add(name);
  if (view && browsingMode) mainWindow.removeBrowserView(view);
  syncControlShield();
});
ipcMain.handle('hide-overlay', (_e, name = 'legacy') => {
  shellOverlays.delete(name);
  const view = getActiveView();
  if (view && browsingMode && !shellOverlays.size) { mainWindow.setBrowserView(view); layoutActiveTab(); }
  if (!shellOverlays.size) sendToRenderer('page-backdrop', null);
});

ipcMain.handle('toggle-dev-panel', (_event, reducedMotion = false) => {
  devPanelOpen = !devPanelOpen;
  clearInterval(panelMotionTimer);
  const from = panelProgress;
  const target = devPanelOpen ? 1 : 0;
  const started = performance.now();
  if (reducedMotion) layoutActiveTab();
  else {
    const tick = () => {
      if (!mainWindow || mainWindow.isDestroyed()) { clearInterval(panelMotionTimer); return; }
      const elapsed = Math.min(1, (performance.now() - started) / 220);
      const eased = 1 - Math.pow(1 - elapsed, 3);
      layoutActiveTab(from + (target - from) * eased);
      if (elapsed === 1) clearInterval(panelMotionTimer);
    };
    panelMotionTimer = setInterval(tick, 16);
    tick();
  }
  sendToRenderer('dev-panel-state', devPanelOpen);
  return devPanelOpen;
});

ipcMain.handle('resize-dev-panel', (e, width) => {
  if (!Number.isFinite(width)) return devPanelWidth;
  devPanelWidth = Math.max(320, Math.min(width, 560));
  config.ui = { ...config.ui, panelWidth: devPanelWidth };
  clearTimeout(panelSaveTimer);
  panelSaveTimer = setTimeout(() => { panelSaveTimer = null; saveConfig(); }, 180);
  layoutActiveTab();
  return devPanelWidth;
});

ipcMain.handle('get-ui-preferences', () => ({ theme: 'system', pane: 'record', ...config.ui, platform: process.platform, systemDark: nativeTheme.shouldUseDarkColors }));
ipcMain.handle('save-ui-preferences', (_e, preferences) => {
  if (!preferences || typeof preferences !== 'object') return false;
  const ui = { ...config.ui };
  if (['system', 'light', 'dark'].includes(preferences.theme)) ui.theme = preferences.theme;
  if (['record', 'chat', 'actions', 'network', 'source'].includes(preferences.pane)) ui.pane = preferences.pane;
  config.ui = ui;
  saveConfig();
  mainWindow?.setBackgroundColor(shellBackground());
  return true;
});
ipcMain.handle('export-playwright', async (_e, payload) => {
  if (typeof payload?.code !== 'string' || payload.code.length > 2_000_000) throw new Error('Invalid Playwright export');
  const name = typeof payload.name === 'string' && /^[\w-]{1,64}$/.test(payload.name) ? payload.name : 'playbook';
  const result = await dialog.showSaveDialog(mainWindow, { title: 'Save Playwright script', defaultPath: name + '.mjs', filters: [{ name: 'JavaScript', extensions: ['mjs'] }] });
  if (result.canceled) return { canceled: true };
  await fs.promises.writeFile(result.filePath, payload.code, { mode: 0o600 });
  return { saved: true };
});
ipcMain.handle('confirm-discard-recording', async () => {
  const result = await dialog.showMessageBox(mainWindow, { type: 'question', title: 'Discard recording?', message: 'Discard these recorded steps?', detail: 'This cannot be undone. Save your playbook first if you want to keep it.', buttons: ['Keep recording', 'Discard'], defaultId: 0, cancelId: 0 });
  return result.response === 1;
});

ipcMain.handle('get-fingerprint', () => {
  if (!activeProfile) return null;
  return {
    id: activeProfile.id,
    platform: activeProfile.navigator.platform,
    hardwareConcurrency: activeProfile.navigator.hardwareConcurrency,
    deviceMemory: activeProfile.navigator.deviceMemory,
    screen: `${activeProfile.screen.width}x${activeProfile.screen.height}`,
    dpr: activeProfile.screen.devicePixelRatio,
    gpu: activeProfile.webgl.unmaskedRenderer,
    timezone: activeProfile.timezone,
    locale: activeProfile.locale,
    fonts: activeProfile.fonts.available.length,
    canvasNoise: activeProfile.canvas.noiseSeed.toFixed(6),
    audioNoise: activeProfile.audio.noiseSeed.toFixed(6),
  };
});

// ─── Dev Panel: Chat, Source & Quick Actions ───

ipcMain.handle('send-chat', async (e, messages) => {
  if (!wsReady || !browserId) return { error: 'Not connected to server' };
  try {
    const res = await fetch(`${serverHttpBase()}/api/browsers/${browserId}/chat`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${config.apiKey}`,
      },
      body: JSON.stringify({ messages }),
    });
    const text = await res.text();
    try {
      return JSON.parse(text);
    } catch {
      return { error: `Server returned ${res.status}: ${text.slice(0, 200)}` };
    }
  } catch (err) {
    return { error: err.message };
  }
});

ipcMain.handle('get-page-source', async () => {
  const view = getActiveView();
  if (!view) return { html: '', markdown: '', url: '' };
  try {
    const html = await worldEval(view, 'document.documentElement.outerHTML');
    let markdown = '';
    try {
      const result = await worldEval(view,
        '(typeof analyzePage === "function") ? analyzePage({}) : null');
      if (result?.ok) markdown = result.data.markdown || '';
    } catch {}
    return { html, markdown, url: view.webContents.getURL() };
  } catch (e) {
    return { html: '', markdown: '', url: '', error: e.message };
  }
});

ipcMain.handle('dev-action', async (e, action, params) => {
  const view = getActiveView();
  if (!view && action !== 'list-tabs') return { ok: false, error: 'No active tab' };
  try {
    if (!['analyze', 'screenshot', 'list-tabs'].includes(action)) requireHumanControl();
    switch (action) {
      case 'analyze': {
        await injectScripts(view);
        return await worldEval(view,
          '(typeof analyzePage === "function") ? analyzePage({}) : { ok: false, error: "Analyzer not loaded" }');
      }
      // Server-internal: the channel CAPTCHA and MFA handling use. It runs in
      // the PAGE's world, not the analyzer's isolated one — clearing a captcha
      // means calling back into the page's own globals
      // (`___grecaptcha_cfg.clients[…].callback` is a function the page
      // defined), which an isolated world cannot see. Not a public command.
      case 'evaluate_raw': {
        return { ok: true, data: { result: await cdpEval(view, String(params?.expression || '')) } };
      }
      case 'screenshot': {
        const r = await cdp(view, 'Page.captureScreenshot', { format: 'png' });
        return { ok: true, data: { screenshot: 'data:image/png;base64,' + r.data } };
      }
      case 'scroll-down': {
        const vp = await cdpEval(view, '({ w: window.innerWidth, h: window.innerHeight })');
        await cdpScroll(view, (vp?.w || 800) / 2, (vp?.h || 600) / 2, 0, params?.amount || 400);
        return { ok: true };
      }
      case 'scroll-up': {
        const vp2 = await cdpEval(view, '({ w: window.innerWidth, h: window.innerHeight })');
        await cdpScroll(view, (vp2?.w || 800) / 2, (vp2?.h || 600) / 2, 0, -(params?.amount || 400));
        return { ok: true };
      }
      case 'reload': {
        view.webContents.reload();
        return { ok: true };
      }
      case 'navigate': {
        if (!params?.url) return { ok: false, error: 'URL required' };
        let url = params.url;
        if (!/^https?:\/\//i.test(url)) url = 'https://' + url;
        await pullCookiesFor(url);
        await view.webContents.loadURL(url);
        await injectScripts(view);
        return { ok: true, data: { url: view.webContents.getURL(), title: view.webContents.getTitle() } };
      }
      case 'click': {
        if (!params?.element_id) return { ok: false, error: 'element_id required' };
        await injectScripts(view);
        const selector = `[data-ac-id="${params.element_id}"]`;
        const info = await worldEval(view, FIND_ELEMENT_JS(selector));
        if (!info?.ok) return { ok: false, error: info?.error || 'Element not found' };
        await cdpClick(view, info.data.x, info.data.y);
        await sleep(300);
        return { ok: true, data: { clicked: true, url: view.webContents.getURL() } };
      }
      case 'type': {
        if (!params?.element_id || !params?.text) return { ok: false, error: 'element_id and text required' };
        await injectScripts(view);
        const sel = `[data-ac-id="${params.element_id}"]`;
        const inf = await worldEval(view, FIND_ELEMENT_JS(sel));
        if (!inf?.ok) return { ok: false, error: inf?.error || 'Element not found' };
        await cdpClick(view, inf.data.x, inf.data.y);
        await sleep(20);
        await cdpClearField(view);
        await cdpTypeText(view, params.text);
        return { ok: true, data: { typed: true } };
      }
      case 'press-key': {
        if (!params?.key) return { ok: false, error: 'key required' };
        await cdpPressKey(view, params.key);
        return { ok: true, data: { key: params.key } };
      }
      case 'hover': {
        if (!params?.element_id) return { ok: false, error: 'element_id required' };
        await injectScripts(view);
        const hSel = `[data-ac-id="${params.element_id}"]`;
        const hInfo = await worldEval(view, FIND_ELEMENT_JS(hSel));
        if (!hInfo?.ok) return { ok: false, error: hInfo?.error || 'Element not found' };
        await cdpMouseMove(view, Math.round(hInfo.data.x), Math.round(hInfo.data.y));
        return { ok: true, data: { hovered: true } };
      }
      case 'click-coords': {
        if (params?.x == null || params?.y == null) return { ok: false, error: 'x and y required' };
        await cdpClick(view, params.x, params.y);
        return { ok: true, data: { clicked: true, x: params.x, y: params.y } };
      }
      case 'wait': {
        if (!params?.selector) return { ok: false, error: 'selector required' };
        await injectScripts(view);
        const wResult = await worldEval(view, `(async () => { const maxWait = ${params?.timeout || 10000}; const start = Date.now(); while (Date.now() - start < maxWait) { if (document.querySelector(${JSON.stringify(params.selector)})) return { ok: true, data: { found: true } }; await new Promise(r => setTimeout(r, 250)); } return { ok: false, error: 'Timeout' }; })()`, true
        );
        return wResult;
      }
      case 'list-tabs': {
        return { ok: true, data: { tabs: tabs.map(t => ({ id: t.id, title: t.title, url: t.url, active: t.id === activeTabId })) } };
      }
      case 'new-tab': {
        const tabId = createTab(params?.url || 'about:blank', true);
        return { ok: true, data: { tab_id: tabId } };
      }
      case 'close-tab': {
        closeTab(params?.tab_id || activeTabId);
        return { ok: true, data: { closed: true } };
      }
      case 'select': {
        if (!params?.element_id || !params?.value) return { ok: false, error: 'element_id and value required' };
        await injectScripts(view);
        const sResult = await worldEval(view, `(() => {
          const el = document.querySelector('[data-ac-id=' + ${JSON.stringify(JSON.stringify(String(params.element_id)))} + ']');
          if (!el || el.tagName !== 'SELECT') return { ok: false, error: 'Select element not found' };
          el.value = ${JSON.stringify(params.value)};
          el.dispatchEvent(new Event('change', { bubbles: true }));
          return { ok: true, data: { selected: el.value } };
        })()`, true);
        return sResult;
      }
      default:
        return { ok: false, error: 'Unknown action: ' + action };
    }
  } catch (err) {
    return { ok: false, error: err.message };
  }
});

function sendToRenderer(channel, data) {
  if (mainWindow && !mainWindow.isDestroyed()) mainWindow.webContents.send(channel, data);
}

// ─── Dev Log ───

function devLog(direction, type, data) {
  if (!mainWindow || mainWindow.isDestroyed()) return;
  const entry = {
    ts: Date.now(), dir: direction, type,
    data: JSON.stringify(redact(data), null, 2),
  };
  if (entry.data && entry.data.length > 8000) entry.data = entry.data.slice(0, 8000) + '\n... (truncated)';
  mainWindow.webContents.send('dev-log', entry);
}

// ─── WebSocket ───

function connect() {
  if (!config.apiKey && process.env.OYA_AUTO_CONNECT !== 'true') return;
  if (ws) disconnect();
  browserId = browserId || randomId();

  try {
    const socket = new WebSocket(config.serverUrl);
    let messageQueue = Promise.resolve();

    socket.on('open', () => {
      devLog('out', 'auth', { browser_id: browserId, browser_name: config.browserName });
      socket.send(JSON.stringify({
        type: 'auth', api_key: config.apiKey,
        browser_id: browserId, browser_name: config.browserName,
        persona: config.persona,
        provider: config.provider || (process.env.OYA_DOCKER ? 'oya-selfhosted' : 'oya-desktop'),
        enrollment_token: process.env.OYA_ENROLLMENT_TOKEN,
        // The server may relay CDP to our front door over this socket.
        cdp: !!CDP_PORT,
      }));
    });

    socket.on('message', (raw) => {
      let msg;
      try { msg = JSON.parse(raw.toString()); } catch { return; }
      if (msg.type !== 'ping' && msg.type !== 'pong') {
        if (msg.type === 'cmd') {
          devLog('in', `cmd: ${msg.action}`, { id: msg.id?.slice(0, 8), action: msg.action, params: msg.params && Object.keys(msg.params).length ? msg.params : '(none)' });
        } else {
          devLog('in', msg.type, msg);
        }
      }
      messageQueue = messageQueue.then(() => handleServerMessage(msg)).catch(() => {
        socket.close(4003, 'Session setup failed');
      });
    });

    socket.on('close', (code) => {
      wsReady = false;
      desktopControl.disconnect();
      closeCdpRelays();
      clearInterval(pingInterval);
      sendStatus();
      // Don't reconnect on fatal/intentional close codes
      if (code === 4000) {
        console.log('[oya] Connection replaced by new session — not reconnecting');
        return;
      }
      if (code === 4001 || code === 4003) {
        console.log('[oya] Auth failure — not reconnecting');
        return;
      }
      scheduleReconnect();
    });
    socket.on('error', () => {});
    ws = socket;
  } catch { scheduleReconnect(); }
}

function disconnect() {
  closeCdpRelays();
  flushCookieChanges();
  stopStream();
  clearTimeout(reconnectTimer); clearInterval(pingInterval);
  reconnectTimer = null; reconnectAttempts = 0; missedPongs = 0;
  if (ws) { ws.removeAllListeners(); try { ws.close(); } catch {} ws = null; }
  wsReady = false; desktopControl.disconnect(); sendStatus();
}

function scheduleReconnect() {
  if (reconnectTimer) return;
  reconnectAttempts++;
  reconnectTimer = setTimeout(() => { reconnectTimer = null; connect(); },
    Math.min(500 * Math.pow(1.5, reconnectAttempts), 10000) + Math.random() * 500);
}

function sendStatus() { sendToRenderer('ws-status', { connected: wsReady, browserId, profileName: config.profileName || 'Default' }); }

ipcMain.handle('save-profile', async () => {
  if (!wsReady || ws?.readyState !== WebSocket.OPEN) throw new Error('Connect the desktop before saving your profile.');
  flushCookieChanges();
  await dumpCookies();
  await getBrowserSession().cookies.flushStore();
  getBrowserSession().flushStorageData();
  wsSend({ type: 'profile_flush' });
});

async function handleServerMessage(msg) {
  switch (msg.type) {
    case 'control_mode':
      if (msg.state) desktopControl.receive(msg.state);
      governance.setMode(msg.state ? desktopControl.snapshot().mode : msg.mode);
      break;
    case 'desktop_control_result':
      desktopControl.result(msg);
      if (msg.state) governance.setMode(desktopControl.snapshot().mode);
      break;
    case 'auth_ok':
      reconnectAttempts = 0;
      if (msg.browser_id) browserId = msg.browser_id;
      if (!loginState || activeProfile?.id !== msg.fingerprint?.id) loginState = new LoginState(msg.origins || {}, (origins) => {
        if (wsReady && ws?.readyState === WebSocket.OPEN) {
          wsSend({ type: 'storage_changed', origins });
        }
      });
      // Apply fingerprint from the server — the server is the single source of truth.
      // Same API key = same fingerprint on every browser, guaranteed.
      if (msg.fingerprint) await applyServerFingerprint(msg.fingerprint, msg.cookies || []);
      wsReady = true;
      desktopControl.connect(msg.control);
      if (msg.control) governance.setMode(msg.control.mode);
      config.profileName = msg.persona?.name || 'Default';
      saveConfig();
      startPingLoop(); sendStatus();
      if (!browsingMode) enterBrowsingMode(governance.configuration ? 'about:blank' : 'https://google.com');
      // Send our cookies to the server for pool sync
      await dumpCookies();
      wsSend({ type: 'profile_flush' });
      break;
    case 'profile_saved':
      sendToRenderer('profile-saved', msg);
      break;
    case 'cookie_sync':
      await applyCookieSync(msg.cookies);
      pendingPulls.get(msg.pullId)?.(true);
      break;
    case 'ping': missedPongs = 0; wsSend({ type: 'pong' }); break;
    case 'pong': missedPongs = 0; break;
    case 'stream_start': startStream(msg.fps || 2); break;
    case 'stream_stop': stopStream(); break;
    case 'cmd': handleCommand(msg); break;
    case 'cdp_open': openCdpRelay(msg.sid); break;
    case 'cdp': cdpRelays.get(msg.sid)?.send(String(msg.data)); break;
    case 'cdp_close': { const sock = cdpRelays.get(msg.sid); cdpRelays.delete(msg.sid); sock?.close(); break; }
  }
}

// ─── CDP relay ───
// The server's gateway reaches our CDP front door through the control socket,
// so a sandbox or a desktop behind NAT needs no inbound port. sid → local socket.
const cdpRelays = new Map();

async function openCdpRelay(sid) {
  const fail = (error) => { cdpRelays.delete(sid); wsSend({ type: 'cdp_closed', sid, error }); };
  if (!CDP_PORT) return fail('CDP is off in this browser. Start it with OYA_REMOTE_DEBUGGING_PORT set.');
  try {
    const { webSocketDebuggerUrl } = await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/version`)).json();
    const sock = new WebSocket(`ws://127.0.0.1:${CDP_PORT}${new URL(webSocketDebuggerUrl).pathname}`, { headers: { 'X-Oya-Relay': CDP_RELAY_TOKEN }, perMessageDeflate: false, maxPayload: 256 * 1024 * 1024 });
    cdpRelays.set(sid, sock);
    sock.on('open', () => wsSend({ type: 'cdp_opened', sid }));
    sock.on('message', (data) => wsSend({ type: 'cdp', sid, data: data.toString() }));
    sock.on('close', () => { if (cdpRelays.delete(sid)) wsSend({ type: 'cdp_closed', sid }); });
    sock.on('error', (e) => { if (cdpRelays.has(sid)) fail(e.message); });
  } catch (e) { fail(e.message); }
}

function closeCdpRelays() {
  for (const sock of cdpRelays.values()) try { sock.close(); } catch {}
  cdpRelays.clear();
}

let proxyBytesUnsent = 0;

function startPingLoop() {
  clearInterval(pingInterval); missedPongs = 0;
  pingInterval = setInterval(() => {
    missedPongs++;
    if (missedPongs > 4) { clearInterval(pingInterval); if (ws) try { ws.close(); } catch {} return; }
    wsSend({ type: 'ping' });
    // Residential proxy traffic since the last beat; kept for the next one if the send fails.
    const proxyBytes = takeProxyBytes();
    if (proxyBytes && !wsSend({ type: 'proxy_bytes', bytes: proxyBytes })) proxyBytesUnsent += proxyBytes;
    else if (proxyBytesUnsent && wsSend({ type: 'proxy_bytes', bytes: proxyBytesUnsent })) proxyBytesUnsent = 0;
  }, 20000);
}

// ─── Command Handling ───

// Shared: find element via injected analyzer, return its center + metadata
const FIND_ELEMENT_JS = (selector) => `(() => {
  window.__oyaInternalCall = true;
  try {
  const f = window.__acFindElement || ((s) => document.querySelector(s));
  const el = f(${JSON.stringify(selector)});
  if (!el) return { ok: false, error: 'Element not found: ${selector.replace(/'/g, "\\'")}' };
  el.scrollIntoView({ behavior: 'instant', block: 'center' });
  // Check if element is behind a sticky header and adjust scroll
  let rect = el.getBoundingClientRect();
  if (rect.top < 80) {
    // Likely behind a sticky nav (LinkedIn, Reddit, HN all have sticky headers ~52-80px)
    window.scrollBy(0, rect.top - 100);
    rect = el.getBoundingClientRect();
  }
  let offsetX = 0, offsetY = 0;
  // If element is inside an iframe, offset by the iframe's position in the parent page
  const ownerDoc = el.ownerDocument;
  if (ownerDoc !== document) {
    for (const iframe of document.querySelectorAll('iframe')) {
      try { if (iframe.contentDocument === ownerDoc) {
        const iframeRect = iframe.getBoundingClientRect();
        offsetX = iframeRect.x;
        offsetY = iframeRect.y;
        break;
      }} catch {}
    }
  }
  return {
    ok: true,
    data: {
      x: rect.x + rect.width / 2 + offsetX,
      y: rect.y + rect.height / 2 + offsetY,
      tag: el.tagName,
      editable: el.isContentEditable,
      inIframe: ownerDoc !== document,
    },
  };
  } finally { window.__oyaInternalCall = false; }
})()`;

/**
 * Wait for a tab's first load before driving it — but never unconditionally.
 *
 * tab.ready only settles once CDP setup and the initial navigation finish. A
 * page that never finishes doing either used to block every later command on
 * that tab with no result and no error, so the caller just timed out. That is
 * how a browser image whose renderers would not start looked like a dead
 * server rather than a broken page.
 */
const TAB_READY_TIMEOUT = 20000;

function waitForTabReady(tab) {
  if (!tab?.ready) return Promise.resolve();
  // A rejected first load is not this command's problem: it is about to
  // navigate somewhere else anyway.
  return Promise.race([tab.ready.catch(() => {}), sleep(TAB_READY_TIMEOUT)]);
}

async function handleCommand(msg) {
  const { id, action, params } = msg;

  if (!browsingMode) { sendResult(id, false, null, 'Browser not ready'); return; }

  try {
    // ── Tab management ──

    // The server drives recording too, so a flow can be demonstrated from the
    // dashboard's live view. Both routes share one buffer: the panel here and the
    // dashboard show the same steps.
    if (action === 'workflow') {
      if (!workspace || workspace.busy() || recording) throw new Error('Finish the active recording or validation before playing a workflow');
      workspace.persist();
      workspace.draft = normalizeDraft({ ...params.draft, id: crypto.randomUUID(), phase: 'paused' });
      workspace.history = []; workspace.future = []; workspace.persist();
      recordedSteps = structuredClone(workspace.draft.steps); recordedSecrets = new Set(workspace.draft.secrets);
      captureSignature = JSON.stringify([false, recordedSteps, [...recordedSecrets]]);
      await workspace.start({ vars: params.variables || {}, autoHeal: params.autoHeal !== false });
      const started = Date.now();
      while (workspace.busy() && Date.now() - started < 540000 && wsReady) await sleep(200);
      if (workspace.busy()) { workspace.session?.dispose(); workspace.receive({ type: 'finished', status: 'interrupted', error: 'Remote validation disconnected or exceeded its time limit. Check the website before retrying.' }); }
      sendResult(id, true, { id: workspace.run.id, status: workspace.run.status, assertions: workspace.run.assertions || 0, error: workspace.run.error });
      return;
    }
    if (action === 'record') {
      const mode = params?.mode;
      const result = await queueRecording(async () => {
        if (mode === 'start') await startRecording(false, 'remote');
        else if (mode === 'stop') await stopRecording();
        else await drainAll();
        return { recording, steps: [...recordedSteps], secrets: [...recordedSecrets] };
      });
      sendResult(id, true, result);
      return;
    }
    if (action === 'list_tabs') {
      sendResult(id, true, { tabs: tabs.map(t => ({ id: t.id, title: t.title, url: t.url, active: t.id === activeTabId })) });
      return;
    }
    if (action === 'open_tab') {
      const tabId = createTab(params?.url || 'about:blank', true);
      await waitForTabReady(tabs.find((t) => t.id === tabId));
      sendResult(id, true, { tab_id: tabId, url: params?.url || 'about:blank' });
      return;
    }
    if (action === 'switch_tab') {
      if (!tabs.find(t => t.id === params?.tab_id)) { sendResult(id, false, null, `Tab ${params?.tab_id} not found`); return; }
      activateTab(params.tab_id);
      sendResult(id, true, { tab_id: params.tab_id });
      return;
    }
    if (action === 'close_tab') {
      closeTab(params?.tab_id || activeTabId);
      sendResult(id, true, { closed: true });
      return;
    }

    // All remaining actions need an active tab
    const view = getActiveView();
    if (!view || view.webContents.isDestroyed()) {
      sendResult(id, false, null, 'No active tab');
      return;
    }

    // ── Navigate ──

    if (action === 'navigate' && params?.url) {
      await waitForTabReady(tabs.find((t) => t.view === view));
      await pullCookiesFor(params.url);
      const maxRetries = 2;
      let lastErr = null;
      for (let attempt = 0; attempt <= maxRetries; attempt++) {
        try {
          await view.webContents.loadURL(params.url);
          lastErr = null;
          break;
        } catch (navErr) {
          if (navErr.message?.includes('ERR_ABORTED')) { lastErr = null; break; }
          lastErr = navErr;
          if (attempt < maxRetries) { await sleep(1000); continue; }
        }
      }
      if (lastErr) { sendResult(id, false, null, lastErr.message); return; }
      await injectScripts(view);
      sendResult(id, true, { url: view.webContents.getURL(), title: view.webContents.getTitle() });
      return;
    }

    // ── Screenshot (CDP) ──

    if (action === 'screenshot') {
      const view = getActiveView();
      const result = await cdp(view, 'Page.captureScreenshot', { format: 'png' });
      sendResult(id, true, { screenshot: 'data:image/png;base64,' + result.data });
      return;
    }

    // ── Click (CDP mouse) ──

    if (action === 'click') {
      const view = getActiveView();
      await injectScripts(view);
      const info = await worldEval(view, FIND_ELEMENT_JS(params?.selector || ''));
      if (!info?.ok) { sendResult(id, false, null, info?.error || 'Element not found'); return; }

      await cdpClick(view, info.data.x, info.data.y);

      // For iframe elements, also dispatch full pointer/mouse event sequence — CDP
      // mouse events may not trigger framework handlers (jsaction, etc.) in iframes.
      if (info.data.inIframe) {
        await worldEval(view, `(() => {
          const f = window.__acFindElement || ((s) => document.querySelector(s));
          const el = f(${JSON.stringify(params?.selector || '')});
          if (!el) return;
          const rect = el.getBoundingClientRect();
          const x = rect.x + rect.width / 2;
          const y = rect.y + rect.height / 2;
          const w = el.ownerDocument.defaultView;
          const opts = { bubbles: true, cancelable: true, view: w, clientX: x, clientY: y, screenX: x, screenY: y };
          el.dispatchEvent(new PointerEvent('pointerdown', { ...opts, pointerId: 1, pointerType: 'mouse' }));
          el.dispatchEvent(new MouseEvent('mousedown', { ...opts, button: 0, buttons: 1 }));
          el.dispatchEvent(new PointerEvent('pointerup', { ...opts, pointerId: 1, pointerType: 'mouse' }));
          el.dispatchEvent(new MouseEvent('mouseup', { ...opts, button: 0 }));
          el.dispatchEvent(new MouseEvent('click', { ...opts, button: 0 }));
        })()`, true).catch(() => {});
      }

      // Wait for potential navigation
      await sleep(300);
      if (view.webContents.isLoading()) {
        await waitForLoad(view);
      }
      const newUrl = view.webContents.getURL();
      const title = view.webContents.getTitle();
      await injectScripts(view);
      sendResult(id, true, { clicked: true, url: newUrl, title });
      return;
    }

    // ── Type (CDP keyboard — human-like) ──

    if (action === 'type') {
      const view = getActiveView();
      await injectScripts(view);

      // Find element and click on it (natural focus — like a human clicking the field)
      const info = await worldEval(view, FIND_ELEMENT_JS(params?.selector || ''));
      if (!info?.ok) { sendResult(id, false, null, info?.error || 'Element not found'); return; }

      await cdpClick(view, info.data.x, info.data.y);
      await sleep(20 + Math.random() * 30);

      const text = params?.text || '';
      if (!text) { sendResult(id, true, { typed: true }); return; }

      if (info.data.inIframe) {
        // CDP keyboard events don't route to iframe frames — use JS clear + Electron insertText
        await worldEval(view, `(() => {
          const f = window.__acFindElement || ((s) => document.querySelector(s));
          const el = f(${JSON.stringify(params?.selector || '')});
          if (!el) return;
          el.focus();
          if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') { el.value = ''; }
          else if (el.isContentEditable) { el.textContent = ''; }
          el.dispatchEvent(new Event('input', { bubbles: true }));
        })()`, true).catch(() => {});
        await sleep(15);

        // Type character by character using insertText (routes to focused iframe element)
        let prev = '';
        for (const ch of text) {
          await view.webContents.insertText(ch);
          await sleep(typingDelay(ch, prev));
          prev = ch;
        }
      } else {
        // Clear existing content — use JS to target the specific element
        // instead of CDP Cmd+A which can select the entire page
        const cleared = await worldEval(view, `(() => {
          const f = window.__acFindElement || ((s) => document.querySelector(s));
          const el = f(${JSON.stringify(params?.selector || '')});
          if (!el) return false;
          el.focus();
          if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') {
            el.select();
            return 'select';
          } else if (el.isContentEditable) {
            const sel = window.getSelection();
            sel.selectAllChildren(el);
            return 'select';
          }
          return false;
        })()`, true).catch(() => false);

        if (cleared === 'select') {
          await sleep(10 + Math.random() * 15);
          await cdpPressKey(view, 'Backspace');
          await sleep(10 + Math.random() * 15);
        }

        // Type with human cadence
        await cdpTypeText(view, text);
      }

      // Wait for autocomplete/suggestions to appear
      await sleep(800);
      await worldEval(view,
        'if (typeof window.__acForcePollState === "function") window.__acForcePollState()'
      ).catch(() => {});

      // Check if suggestions/autocomplete appeared
      await injectScripts(view);
      const hasDropdown = await worldEval(view, `(() => {
        const lists = document.querySelectorAll('[role="listbox"], [role="menu"], [role="list"], .pac-container, [class*="suggest"], [class*="autocomplete"], [class*="dropdown"], [id*="suggest"], [id*="autocomplete"], ul[class*="result"]');
        for (const l of lists) {
          const r = l.getBoundingClientRect();
          if (r.width > 0 && r.height > 0) return true;
        }
        return false;
      })()`, true).catch(() => false);

      sendResult(id, true, { typed: true, suggestions_visible: hasDropdown });
      return;
    }

    // ── Press key (CDP keyboard) ──

    if (action === 'press_key') {
      const view = getActiveView();
      const key = params?.key || 'Enter';
      // Block dangerous keys that zoom, open emoji picker, or trigger OS shortcuts
      const BLOCKED_KEYS = new Set([
        'F11', 'F12', 'F5',
        'Meta', 'Control', 'Alt', 'Shift',  // bare modifier keys
        'ZoomIn', 'ZoomOut', 'BrowserBack', 'BrowserForward',
        'MediaPlayPause', 'MediaTrackNext', 'MediaTrackPrevious',
        'AudioVolumeUp', 'AudioVolumeDown', 'AudioVolumeMute',
      ]);
      if (BLOCKED_KEYS.has(key)) {
        sendResult(id, false, null, `Key "${key}" is blocked — it can change browser state`);
        return;
      }
      // Use sendInputEvent (routes to focused frame) instead of CDP (main frame only)
      const def = keyDef(key);
      view.webContents.sendInputEvent({ type: 'keyDown', keyCode: def.key });
      await sleep(20 + Math.random() * 30);
      view.webContents.sendInputEvent({ type: 'keyUp', keyCode: def.key });

      if (key === 'Enter') {
        await sleep(300);
        if (view.webContents.isLoading()) {
          await waitForLoad(view);
          await injectScripts(view);
        }
      }
      sendResult(id, true, { key });
      return;
    }

    // ── Click at coordinates (CDP mouse) ──

    if (action === 'click_coordinates') {
      const view = getActiveView();
      const x = params?.x ?? 0;
      const y = params?.y ?? 0;
      await cdpClick(view, x, y);
      await sleep(100);
      const newUrl = view.webContents.getURL();
      const title = view.webContents.getTitle();
      sendResult(id, true, { clicked: true, x, y, url: newUrl, title });
      return;
    }

    // ── Mouse move (CDP mouse) ──

    if (action === 'mouse_move') {
      const view = getActiveView();
      const x = params?.x ?? 0;
      const y = params?.y ?? 0;
      await cdpMouseMove(view, x, y);
      sendResult(id, true, { moved: true, x, y });
      return;
    }

    // ── Double click at coordinates (CDP mouse) ──

    if (action === 'double_click') {
      const view = getActiveView();
      let x, y;
      if (params?.x !== undefined && params?.y !== undefined) {
        x = params.x;
        y = params.y;
      } else if (params?.selector) {
        await injectScripts(view);
        const info = await worldEval(view, FIND_ELEMENT_JS(params.selector));
        if (!info?.ok) { sendResult(id, false, null, info?.error || 'Element not found'); return; }
        x = info.data.x;
        y = info.data.y;
      } else {
        sendResult(id, false, null, 'Provide x,y coordinates or element_id'); return;
      }
      await cdpMouseMove(view, x, y);
      await sleep(10 + Math.random() * 15);
      await cdp(view, 'Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', buttons: 1, clickCount: 1 });
      await cdp(view, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 });
      await sleep(60 + Math.random() * 40);
      await cdp(view, 'Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', buttons: 1, clickCount: 2 });
      await cdp(view, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 2 });
      await sleep(100);
      sendResult(id, true, { double_clicked: true, x, y });
      return;
    }

    // ── Keyboard type raw text (CDP keyboard, no element focus) ──

    if (action === 'keyboard_type') {
      const view = getActiveView();
      const text = params?.text || '';
      if (!text) { sendResult(id, true, { typed: true }); return; }
      await cdpTypeText(view, text);
      sendResult(id, true, { typed: true, text });
      return;
    }

    // ── Drag (CDP mouse) ──

    if (action === 'drag') {
      const view = getActiveView();
      const fromX = params?.from_x ?? 0;
      const fromY = params?.from_y ?? 0;
      const toX = params?.to_x ?? 0;
      const toY = params?.to_y ?? 0;
      await cdpMouseMove(view, fromX, fromY);
      await sleep(10 + Math.random() * 15);
      await cdp(view, 'Input.dispatchMouseEvent', { type: 'mousePressed', x: fromX, y: fromY, button: 'left', buttons: 1 });
      await sleep(30);
      // Move along path
      const steps = 10;
      for (let i = 1; i <= steps; i++) {
        const t = i / steps;
        const x = Math.round(fromX + (toX - fromX) * t);
        const y = Math.round(fromY + (toY - fromY) * t);
        await cdp(view, 'Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, button: 'left', buttons: 1 });
        await sleep(10);
      }
      await cdp(view, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x: toX, y: toY, button: 'left' });
      sendResult(id, true, { dragged: true, from: { x: fromX, y: fromY }, to: { x: toX, y: toY } });
      return;
    }

    // ── Scroll (CDP mouse wheel) ──

    if (action === 'scroll') {
      const view = getActiveView();
      // Live control already has a pointer position and a trackpad delta.
      // Dispatch it once: synthetic smoothing and page analysis add hundreds
      // of milliseconds and cause successive gestures to overlap.
      if (params?.smooth === false && Number.isFinite(params?.x) && Number.isFinite(params?.y)) {
        const amount = Math.max(0, Number(params.amount) || 0);
        await cdpScroll(view, params.x, params.y, 0, params.direction === 'up' ? -amount : amount);
        sendResult(id, true, { direction: params.direction, amount });
        return;
      }
      await injectScripts(view);
      const vp = await cdpEval(view, `({ w: window.innerWidth, h: window.innerHeight })`);
      const cx = Math.round((vp?.w || 800) / 2);
      const cy = Math.round((vp?.h || 600) / 2);
      const amount = params?.amount || 500;
      const delta = params?.direction === 'up' ? -amount : amount;

      // Smooth scroll: break into smaller increments
      const steps = Math.max(3, Math.round(Math.abs(delta) / 120));
      const stepDelta = delta / steps;
      for (let i = 0; i < steps; i++) {
        await cdpScroll(view, cx, cy, 0, stepDelta);
        await sleep(30 + Math.random() * 30);
      }
      await sleep(300);

      // Run analyzer after scroll if requested
      const result = await worldEval(view, `(typeof analyzePage === 'function') ? analyzePage(${JSON.stringify(params?.analyze || {})}) : { ok: true, data: { direction: ${JSON.stringify(String(params?.direction || 'down'))}, amount: ${amount} } }`, true
      );
      sendResult(id, result?.ok ?? true, result?.data, result?.error);
      return;
    }

    // ── Hover (CDP mouse move) ──

    if (action === 'hover') {
      const view = getActiveView();
      await injectScripts(view);
      const info = await worldEval(view, FIND_ELEMENT_JS(params?.selector || ''));
      if (!info?.ok) { sendResult(id, false, null, info?.error || 'Element not found'); return; }
      await cdpMouseMove(view, Math.round(info.data.x), Math.round(info.data.y));
      await sleep(100);
      sendResult(id, true, { hovered: true });
      return;
    }

    // ── Select option (CDP: click select, then click option) ──

    if (action === 'select') {
      const view = getActiveView();
      await injectScripts(view);
      const result = await worldEval(view, `(() => {
        const f = window.__acFindElement || ((s) => document.querySelector(s));
        const el = f(${JSON.stringify(params?.selector || '')});
        if (!el || el.tagName !== 'SELECT') return { ok: false, error: 'Select element not found' };
        el.value = ${JSON.stringify(params?.value || '')};
        el.dispatchEvent(new Event('change', { bubbles: true }));
        el.dispatchEvent(new Event('input', { bubbles: true }));
        return { ok: true, data: { selected: el.value } };
      })()`, true);
      sendResult(id, result?.ok ?? true, result?.data, result?.error);
      return;
    }

    // ── Server-internal: the channel CAPTCHA and MFA handling use ──
    //
    // Runs in the PAGE's world, not the analyzer's isolated one: clearing a
    // captcha means calling back into globals the page defined
    // (`___grecaptcha_cfg.clients[…].callback`), which an isolated world
    // cannot see. Not a public command — /browsers/:id/command rejects it,
    // and only captcha.js and mfa.js reach it.
    if (action === 'evaluate_raw') {
      sendResult(id, true, { result: await cdpEval(view, String(params?.expression || '')) });
      return;
    }

    // ── All other actions via injected scripts ──

    await injectScripts(view);
    const result = await worldEval(view, buildActionJS(action, params));
    sendResult(id, result?.ok ?? true, result?.data, result?.error);
  } catch (err) {
    sendResult(id, false, null, err.message || String(err));
  }
}

function buildActionJS(action, params) {
  switch (action) {
    case 'analyze':
      return `(typeof analyzePage === 'function') ? analyzePage(${JSON.stringify(params || {})}) : { ok: false, error: 'Analyzer not loaded' }`;
    case 'wait':
      return `(async () => { const f = window.__acFindElement || ((s) => document.querySelector(s)); const maxWait = ${params?.timeout || 10000}; const start = Date.now(); while (Date.now() - start < maxWait) { if (f(${JSON.stringify(params?.selector || '')})) return { ok: true, data: { found: true } }; await new Promise(r => setTimeout(r, 250)); } return { ok: false, error: 'Timeout' }; })()`;
    case 'read_page':
      return `({ ok: true, data: { url: location.href, title: document.title, elements: [] } })`;
    default:
      return `({ ok: false, error: 'Unknown action: ${action}' })`;
  }
}

function sendResult(id, ok, data, error) {
  if (!ws || ws.readyState !== WebSocket.OPEN) return;
  const msg = { type: 'cmd_result', id, ok, data: data || null, error: error || null };
  const summary = { id: id.slice(0, 8), ok };
  if (error) summary.error = error;
  if (data) {
    if (data.screenshot) summary.screenshot = `${Math.round(data.screenshot.length / 1024)}KB`;
    if (data.url) summary.url = data.url;
    if (data.title) summary.title = data.title;
    if (data.markdown) summary.markdown = data.markdown.slice(0, 500) + (data.markdown.length > 500 ? '...' : '');
    if (data.elements) summary.elements = `${data.elements.length} elements`;
    if (data.tabs) summary.tabs = `${data.tabs.length} tabs`;
    if (data.tab_id) summary.tab_id = data.tab_id;
    if (data.viewport) summary.viewport = data.viewport;
    if (data.scroll) summary.scroll = data.scroll;
  }
  devLog('out', ok ? 'result: ok' : 'result: error', summary);
  wsSend(msg);
}

/**
 * Settle when the page finishes or fails loading, or give up.
 *
 * Every listener is removed on the way out. The old version attached one per
 * call and dropped it only when the load fired, so an agent session driving a
 * page that never finishes piled them onto the same webContents.
 */
function waitForLoad(view = getActiveView(), ms = 30000) {
  if (!view || view.webContents.isDestroyed()) return Promise.resolve();
  return new Promise((resolve) => {
    const done = () => {
      clearTimeout(timer);
      if (!view.webContents.isDestroyed()) {
        view.webContents.off('did-finish-load', done);
        view.webContents.off('did-fail-load', done);
      }
      resolve();
    };
    const timer = setTimeout(done, ms);
    view.webContents.once('did-finish-load', done);
    view.webContents.once('did-fail-load', done);
  });
}

// ─── Live Stream ───

let streamInterval = null;
let streamCapturing = false;
function startStream(fps) {
  stopStream();
  const ms = Math.max(200, Math.round(1000 / fps));
  streamInterval = setInterval(async () => {
    const view = getActiveView();
    if (!ws || ws.readyState !== WebSocket.OPEN || !view || streamCapturing || ws.bufferedAmount > 1024 * 1024) return;
    streamCapturing = true;
    try {
      const img = await view.webContents.capturePage();
      // capturePage() returns device pixels — 2x the page on a retina screen. The
      // live view maps a click through the frame's own width and sends it as CSS
      // pixels, so an unscaled frame puts every click at twice the distance from
      // the top-left: near enough at the corner, nowhere near the target at the
      // other edge. Send the page at the size the page thinks it is.
      const { width, height } = view.getBounds();
      const frame = width > 0 && img.getSize().width !== width ? img.resize({ width, height, quality: 'good' }) : img;
      wsSend({ type: 'frame', data: 'data:image/jpeg;base64,' + frame.toJPEG(40).toString('base64') });
    } catch {} finally { streamCapturing = false; }
  }, ms);
}
function stopStream() { if (streamInterval) { clearInterval(streamInterval); streamInterval = null; } }

// ─── Utils ───

function randomId() {
  return 'oya-' + Array.from(crypto.getRandomValues(new Uint8Array(8)))
    .map(b => b.toString(16).padStart(2, '0')).join('');
}
