/**
 * Oya Browser, Desktop browser with agent scripts built in.
 * Users run this on their machines. Connects to a deployed Oya server.
 * Multi-tab, persistent cookies, real browser, no extension install needed.
 *
 * All user input (click, type, key press, scroll) goes through Chrome DevTools
 * Protocol for full native control. Human-like timing and mouse paths.
 *
 * This file is the composition root: it sets the process-wide flags that must
 * precede `ready`, builds each main-process service once into `ctx`, and wires
 * Electron's events to them. The services live in main/ (see ARCHITECTURE.md).
 */
require('./governance');
const electron = require('electron');
const path = require('path');
const fs = require('fs');
const crypto = require('crypto');
const { applyTelemetryFlags } = require('./anonymity/telemetry');
const { applyDNSLeakPrevention } = require('./anonymity/proxy');
const { createControlState } = require('./control-state.cjs');
const { cdp } = require('./main/cdp.cjs');
const { createWorld } = require('./main/world.cjs');
const { Observer } = require('./main/observe/observer.cjs');
const { createCookieSync, cookieSyncMark } = require('./main/cookie-sync.cjs');
const { createUpdater } = require('./main/updater.cjs');
const { Routines } = require('./main/routines.cjs');
const { createPageActions } = require('./main/page-actions.cjs');
const { createStream } = require('./main/stream.cjs');
const { createCdpRelay } = require('./main/cdp-relay.cjs');
const { createMirror } = require('./main/mirror/index.cjs');
const { ConfigStore } = require('./main/app/config-store.cjs');
const { Persona } = require('./main/app/persona.cjs');
const { DeepLinks } = require('./main/app/deep-links.cjs');
const { bootBrowser, Lifecycle } = require('./main/app/lifecycle.cjs');
const { ShellWindow } = require('./main/shell/window.cjs');
const { Shortcuts } = require('./main/shell/shortcuts.cjs');
const { ControlShield } = require('./main/shell/control-shield.cjs');
const { PanelLayout } = require('./main/shell/layout.cjs');
const { Overlays } = require('./main/shell/overlays.cjs');
const { TabManager } = require('./main/tabs/tabs.cjs');
const { Protection } = require('./main/tabs/protection.cjs');
const { Recorder } = require('./main/recording/recorder.cjs');
const { ControlSocket } = require('./main/connection/socket.cjs');
const { CommandRunner } = require('./main/connection/commands.cjs');
const { createHandle } = require('./main/ipc/handle.cjs');
const { registerIpc } = require('./main/ipc/index.cjs');

const { app, nativeImage } = electron;
// Branding must not move existing cookies, profiles, or saved settings.
const desktopUserDataPath = app.getPath('userData');
app.setName('Oya Browser');
app.setPath('userData', desktopUserDataPath);

if (process.env.OYA_USER_DATA_DIR) app.setPath('userData', path.resolve(process.env.OYA_USER_DATA_DIR));
// CDP for automation harnesses. Off unless asked for: whoever reaches this port
// owns the browser. Chromium listens one port up on loopback; cdp-front-door.js
// owns the public port and shows harnesses only real, protected tabs. No
// remote-allow-origins, CDP clients send no Origin, and allowing one would
// let any web page on the machine drive it.
const CDP_PORT = Number(process.env.OYA_REMOTE_DEBUGGING_PORT) || 0;
/** Random bytes in the relay token and the isolated world's name. */
const RELAY_TOKEN_BYTES = 32;
const WORLD_NAME_BYTES = 8;
const CDP_RELAY_TOKEN = crypto.randomBytes(RELAY_TOKEN_BYTES).toString('hex');
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

// Set dock icon on macOS (needed for dev mode, built app uses icon from package.json)
//
// icon_1024, not icon.png: the latter is the 6250x6250 master electron-builder
// resizes at build time. Handing it to nativeImage decodes 156MB per
// representation, and macOS makes several, it was 596MB of CG image data in
// the main process, most of this app's memory, for a dock icon.
if (process.platform === 'darwin') {
  const iconPath = path.join(__dirname, 'build', 'icon_1024.png');
  if (fs.existsSync(iconPath)) app.whenReady().then(() => app.dock.setIcon(nativeImage.createFromPath(iconPath)));
}

// ─── The context: every main-process service, built once ───

const analyzerScript = fs.readFileSync(path.join(__dirname, 'scripts', 'analyzer.js'), 'utf8');
const ISOLATED_WORLD = 'w' + crypto.randomBytes(WORLD_NAME_BYTES).toString('hex');

/** Every service, by name; each one reaches the others through it. */
// What the page said and fetched, collected in this process so an agent can read
// it back without a CDP domain the page could detect (see main/observe/).
const observer = new Observer();
const ctx = {
  electron,
  analyzerScript,
  isolatedWorld: ISOLATED_WORLD,
  cdpPort: CDP_PORT,
  relayToken: CDP_RELAY_TOKEN,
  observer,
};
ctx.workspace = null;
ctx.config = new ConfigStore({ dir: () => app.getPath('userData') });
ctx.shell = new ShellWindow(ctx);
ctx.shortcuts = new Shortcuts(ctx);
ctx.shield = new ControlShield(ctx);
ctx.layout = new PanelLayout(ctx);
ctx.overlays = new Overlays(ctx);
ctx.tabs = new TabManager(ctx);
ctx.protection = new Protection(ctx);
ctx.persona = new Persona(ctx);
ctx.recorder = new Recorder(ctx);
ctx.socket = new ControlSocket(ctx);
ctx.commands = new CommandRunner(ctx);
ctx.deepLinks = new DeepLinks(ctx);
ctx.routines = new Routines(ctx);
ctx.world = createWorld({ cdp, analyzerScript, worldName: ISOLATED_WORLD });
ctx.control = createControlState({
  send: (message) => ctx.socket.send(message),
  changed: (state) => ctx.shield.controlChanged(state),
});
ctx.cookies = createCookieSync({
  mark: cookieSyncMark(ctx),
  session: () => ctx.persona.session(),
  send: (payload) => ctx.socket.send(payload),
  open: () => ctx.socket.isOpen(),
  ready: () => ctx.socket.ready,
});
ctx.relay = createCdpRelay({ port: CDP_PORT, token: CDP_RELAY_TOKEN, send: (payload) => ctx.socket.send(payload) });
ctx.mirror = createMirror(ctx);
ctx.stream = createStream({
  activeView: () => ctx.tabs.getActiveView(),
  socket: () => ctx.socket.ws,
  send: (payload) => ctx.socket.send(payload),
});
ctx.actions = createPageActions({
  config: ctx.config,
  navigate: (url) => ctx.tabs.navigateActive(url),
  getActiveView: () => ctx.tabs.getActiveView(),
  pullCookiesFor: (...args) => ctx.cookies.pullCookiesFor(...args),
  injectScripts: (view) => ctx.protection.injectScripts(view),
  worldEval: (...args) => ctx.world.worldEval(...args),
  sendResult: (...args) => ctx.commands.sendResult(...args),
  createTab: (...args) => ctx.tabs.createTab(...args),
  closeTab: (...args) => ctx.tabs.closeTab(...args),
  requireHumanControl: () => ctx.shield.requireHumanControl(),
  analysisStarted: (view) => ctx.shield.analysisStarted(view),
  analysisFinished: (view, raw) => ctx.shield.analysisFinished(view, raw),
  tabs: () => ctx.tabs.list,
  activeTabId: () => ctx.tabs.activeTabId,
});

// ─── Events ───

// Windows and Linux deliver the link as an argv entry to a second launch;
// without the single-instance lock that launch becomes a second browser with
// its own session, and the cookies land in the wrong place.
if (!app.requestSingleInstanceLock()) app.quit();
else app.on('second-instance', (_event, argv) => ctx.deepLinks.onSecondInstance(argv));
app.on('open-url', (event, url) => ctx.deepLinks.onOpenUrl(event, url));

const handle = createHandle(ctx);
const lifecycle = new Lifecycle(ctx);
ctx.startAutoUpdate = createUpdater({
  handle,
  sendToRenderer: (channel, data) => ctx.shell.send(channel, data),
  beforeInstall: () => lifecycle.flushJar(),
}).startAutoUpdate;
registerIpc(handle, ctx);

app.whenReady().then(() => bootBrowser(ctx));
lifecycle.install();
