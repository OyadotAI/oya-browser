/**
 * Outbound CDP driver.
 *
 * Drives any browser that exposes a Chrome DevTools Protocol endpoint: Anchor,
 * Browserbase, Steel, Hyperbrowser, or a plain Chrome started with
 * --remote-debugging-port. The control plane dials out, which is the opposite
 * direction to the Oya client that dials in.
 *
 * Capability parity with the Oya client comes from injecting the same
 * scripts/analyzer.js into the page, so analyze and click-by-element_id behave
 * identically rather than degrading to raw coordinates.
 */

import WebSocket from 'ws';
import { readFileSync } from 'fs';
import { randomBytes } from 'crypto';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { createRequire } from 'module';
import { userAgentFor, metadataFor } from '../ua.js';

const require = createRequire(import.meta.url);
const { LoginState, cdpCookies } = require('../../../browser/login-state.js');

/**
 * Anchor, Browserbase, Steel and Browser Use ship tuned stealth of their own.
 * Layering ours on top produces contradictions that are themselves detectable,
 * so the injection is for browsers nobody else has already treated.
 */
const PROVIDER_SHIPS_STEALTH = new Set(['anchor', 'browserbase', 'steel', 'browseruse']);

let applierFactory;
function getApplier() {
  if (applierFactory === undefined) {
    try { ({ createPersonaApplier: applierFactory } = require('../../../browser/anonymity/apply.js')); }
    catch (e) {
      applierFactory = null;
      console.warn(`[cdp] anonymity/apply.js not found (${e.message}) — CDP browsers run unspoofed`);
    }
  }
  return applierFactory;
}

const __dirname = dirname(fileURLToPath(import.meta.url));

let analyzerScript = null;
let analyzerMissing = false;
function getAnalyzer() {
  if (analyzerScript || analyzerMissing) return analyzerScript;
  try {
    analyzerScript = readFileSync(join(__dirname, '..', '..', '..', 'browser', 'scripts', 'analyzer.js'), 'utf8');
  } catch {
    analyzerMissing = true;
    console.warn('[cdp] analyzer.js not found — analyze/click-by-id unavailable for CDP browsers');
  }
  return analyzerScript;
}

/**
 * Analyzer element ids are integers assigned by analyzePage(). Rejecting
 * anything else keeps caller-supplied values out of evaluated source, rather
 * than relying on every interpolation site escaping correctly.
 */
function elementSelector(elementId) {
  // Only a number or a numeric string. An object with a coercing toString()
  // would slip past Number() alone.
  const id = (typeof elementId === 'number' || typeof elementId === 'string') ? Number(elementId) : NaN;
  if (!Number.isInteger(id) || id < 0) {
    throw Object.assign(new Error('element_id must be an analyzer element id'), { status: 400 });
  }
  return `[data-ac-id="${id}"]`;
}

const FIND_ELEMENT_JS = (selector) => `(() => {
  {
    const f = window.__acFindElement || ((s) => document.querySelector(s));
    const el = f(${JSON.stringify(selector)});
    if (!el) return { ok: false, error: 'Element not found' };
    el.scrollIntoView({ behavior: 'instant', block: 'center' });
    let r = el.getBoundingClientRect();
    if (r.top < 80) { window.scrollBy(0, r.top - 100); r = el.getBoundingClientRect(); }
    return { ok: true, data: { x: r.left + r.width / 2, y: r.top + r.height / 2 } };
  }
})()`;

/** Minimal CDP JSON-RPC transport over the ws dependency we already have. */
export class CDPConnection {
  constructor(url) {
    this.url = url;
    this.nextId = 1;
    this.pending = new Map();
    this.listeners = new Map();
    this.closed = false;
  }

  connect(timeoutMs = 15000) {
    return new Promise((resolve, reject) => {
      let settled = false;
      const finish = (err) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        err ? reject(err) : resolve(this);
      };
      const timer = setTimeout(() => finish(new Error('CDP connect timed out')), timeoutMs);

      this.ws = new WebSocket(this.url, { maxPayload: 256 * 1024 * 1024, handshakeTimeout: timeoutMs });
      this.ws.on('open', () => finish());
      this.ws.on('error', (e) => { this.failAll(e); finish(e); });
      this.ws.on('close', () => { this.closed = true; this.failAll(new Error('CDP connection closed')); });
      this.ws.on('message', (raw) => this.onMessage(raw));
    });
  }

  onMessage(raw) {
    let msg;
    try { msg = JSON.parse(raw.toString()); } catch { return; }
    if (msg.id != null) {
      const p = this.pending.get(msg.id);
      if (!p) return;
      clearTimeout(p.timer);
      this.pending.delete(msg.id);
      if (msg.error) p.reject(new Error(msg.error.message || 'CDP error'));
      else p.resolve(msg.result);
      return;
    }
    if (msg.method) {
      for (const fn of this.listeners.get(msg.method) || []) {
        try { fn(msg.params, msg.sessionId); } catch {}
      }
    }
  }

  failAll(err) {
    for (const [, p] of this.pending) { clearTimeout(p.timer); p.reject(err); }
    this.pending.clear();
  }

  on(method, fn) {
    if (!this.listeners.has(method)) this.listeners.set(method, new Set());
    this.listeners.get(method).add(fn);
    return () => this.listeners.get(method)?.delete(fn);
  }

  /** Wait for one occurrence of a CDP event, or resolve null on timeout. */
  once(method, timeoutMs) {
    return new Promise((resolve) => {
      const timer = setTimeout(() => { off(); resolve(null); }, timeoutMs);
      const off = this.on(method, (params) => { clearTimeout(timer); off(); resolve(params); });
    });
  }

  send(method, params = {}, sessionId, timeoutMs = 30000) {
    if (this.closed || this.ws?.readyState !== 1) return Promise.reject(new Error('CDP connection closed'));
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`CDP ${method} timed out`));
      }, timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      try {
        this.ws.send(JSON.stringify(sessionId ? { id, method, params, sessionId } : { id, method, params }));
      } catch (e) {
        clearTimeout(timer);
        this.pending.delete(id);
        reject(e);
      }
    });
  }

  close() {
    this.closed = true;
    try { this.ws?.close(); } catch {}
  }
}

const KEY_CODES = {
  Enter: { key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, text: '\r' },
  Tab: { key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9 },
  Backspace: { key: 'Backspace', code: 'Backspace', windowsVirtualKeyCode: 8 },
  Delete: { key: 'Delete', code: 'Delete', windowsVirtualKeyCode: 46 },
  Escape: { key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 },
  ArrowUp: { key: 'ArrowUp', code: 'ArrowUp', windowsVirtualKeyCode: 38 },
  ArrowDown: { key: 'ArrowDown', code: 'ArrowDown', windowsVirtualKeyCode: 40 },
  ArrowLeft: { key: 'ArrowLeft', code: 'ArrowLeft', windowsVirtualKeyCode: 37 },
  ArrowRight: { key: 'ArrowRight', code: 'ArrowRight', windowsVirtualKeyCode: 39 },
};

export const CDP_CAPABILITIES = new Set([
  'navigate', 'reload', 'back', 'forward', 'screenshot', 'analyze', 'read_page',
  'record',
  'click', 'click-coords', 'hover', 'type', 'select', 'wait', 'cookies',
  // Both spellings, because normalise() accepts both. A caller checking this
  // set must not conclude that press_key is unsupported when it is.
  'press-key', 'press_key', 'click_coordinates', 'mouse_move', 'double_click', 'drag', 'keyboard_type',
  'scroll', 'scroll-up', 'scroll-down', 'scroll-top', 'scroll-bottom',
  'list-tabs', 'list_tabs', 'new-tab', 'open_tab', 'switch-tab', 'switch_tab',
  'close-tab', 'close_tab',
]);

/**
 * One action vocabulary, two spellings. Underscored names come from the Oya
 * client and the agent tools; hyphenated ones are this driver's own.
 */
const ACTION_ALIASES = {
  click_coordinates: 'click-coords',
  press_key: 'press-key',
  list_tabs: 'list-tabs',
  open_tab: 'new-tab',
  new_tab: 'new-tab',
  switch_tab: 'switch-tab',
  close_tab: 'close-tab',
  read_elements: 'read_page',
};

function normalise(action, params = {}) {
  // `scroll` carries its direction in the params; this driver has it in the name.
  if (action === 'scroll') {
    const direction = String(params.direction || 'down');
    return { action: `scroll-${direction}`, params };
  }
  const mapped = ACTION_ALIASES[action] || action;
  // The Oya client names tabs `tab_id`; every Target.* call here wants `id`.
  const id = params.tab_id ?? params.id;
  return { action: mapped, params: id === undefined ? params : { ...params, id } };
}

export class CDPDriver {
  clientType = 'cdp';
  capabilities = CDP_CAPABILITIES;

  constructor({ wsUrl, provider = 'cdp', onClose, fingerprint, login, onStorage } = {}) {
    Object.assign(this, { wsUrl, provider, onClose });
    // The persona carries no UA of its own — it is derived from the real
    // browser's version and this persona's platform in attach(), once the
    // connection can report that version. Reading fingerprint.userAgent here
    // left this null for every persona, so the override never fired and CDP
    // browsers kept HeadlessChrome in the UA and in the request header.
    this.fingerprint = fingerprint || null;
    this.worldName = 'w' + randomBytes(8).toString('hex');
    this.acceptLanguage = fingerprint?.navigator?.languages?.join(',') || null;
    this.login = login;
    this.loginState = login ? new LoginState(login.origins || {}, onStorage) : null;
  }

  async connect() {
    this.conn = await new CDPConnection(this.wsUrl).connect();
    this.personaApply = null;   // bound to the connection it listens on
    this.conn.ws.on('close', () => { try { this.onClose?.(); } catch {} });

    // Attach to a page target, creating one if the browser has none.
    const { targetInfos = [] } = await this.conn.send('Target.getTargets');
    let page = targetInfos.find((t) => t.type === 'page');
    if (!page) {
      const { targetId } = await this.conn.send('Target.createTarget', { url: 'about:blank' });
      page = { targetId };
    }
    await this.attach(page.targetId);
    if (this.login?.cookies?.length) await this.conn.send('Network.setCookies', { cookies: cdpCookies(this.login.cookies) }, this.sessionId);
    return this;
  }

  async attach(targetId) {
    const { sessionId } = await this.conn.send('Target.attachToTarget', { targetId, flatten: true });
    this.sessionId = sessionId;
    this.targetId = targetId;
    this.analyzerLoaded = false;
    this.recording = false;
    this.recorded = [];
    this.recordedSecrets = new Set();
    for (const domain of ['Page', 'Runtime', 'DOM', 'Network']) {
      await this.conn.send(`${domain}.enable`, {}, sessionId).catch(() => {});
    }
    // The UA is an HTTP header as well as a JS property, so it cannot be fixed
    // from an injected script — a page reads HeadlessChrome from the header no
    // matter what navigator.userAgent says. Emulation sets both.
    // A vendor that ships tuned stealth owns the whole surface: our UA,
    // timezone and patches together would contradict theirs, and a
    // contradiction is a stronger signal than either alone. The persona still
    // governs that session's cookie jar, proxy and concurrency there — only
    // the device spoofing is theirs to do.
    if (this.fingerprint && !PROVIDER_SHIPS_STEALTH.has(this.provider)) {
      let userAgent = null;
      try {
        const version = await this.conn.send('Browser.getVersion');
        // Read the browser's own brand list first. The GREASE entry
        // ("Not?A_Brand" and friends) changes between releases, so reusing it
        // beats constructing one — and overriding without any metadata blanks
        // client hints, which is itself a tell.
        const brands = await this.conn.send('Runtime.evaluate', {
          expression: 'JSON.stringify(navigator.userAgentData?.brands || [])',
          returnByValue: true,
        }, sessionId).then((r) => { try { return JSON.parse(r.result?.value || '[]'); } catch { return []; } })
          .catch(() => []);

        this.userAgent = userAgentFor(this.fingerprint, version.userAgent);
        userAgent = {
          userAgent: this.userAgent,
          ...(this.acceptLanguage ? { acceptLanguage: this.acceptLanguage } : {}),
          platform: this.fingerprint.navigator?.platform || undefined,
          userAgentMetadata: metadataFor(this.fingerprint, version.userAgent, brands),
        };
      } catch (e) {
        console.warn(`[cdp] user agent override failed (${e.message}) — this browser reports its real UA`);
      }
      // The persona, applied exactly as test-stealth.js measures it: native
      // emulation, the injection, and every worker and cross-site iframe. One
      // applier per connection; service workers are browser-wide, so once.
      const create = getApplier();
      if (create && !this.personaApply) {
        this.personaApply = create({
          send: (method, params, sid) => this.conn.send(method, params, sid),
          on: (event, fn) => this.conn.on(event, fn),
          profile: this.fingerprint,
          userAgent,
          onError: (what, err) => console.warn(`[cdp] ${what} failed — not covered: ${err.message}`),
        });
        await this.personaApply.browser();
      }
      if (this.personaApply) await this.personaApply.page(sessionId);
    }

    // The analyzer lives in an isolated world, never the page's. In the main
    // world its globals — analyzePage, __acFindElement, __acAnalyzerLoaded —
    // are a one-line, 100%-precision detector for this product, which is worth
    // more to a defender than every other signal on the page combined. Same
    // per-session random tag attribute as the desktop path.
    this.tagAttr = 'data-' + randomBytes(4).toString('hex');
    this.worldContext = null;
    if (this.loginState) {
      await this.loginState.attach(
        (method, params = {}) => this.conn.send(method, params, sessionId),
        (method, fn) => this.conn.on(method, (params, sid) => { if (sid === sessionId) fn(params); }),
      );
    }
  }

  isAlive() { return !!this.conn && !this.conn.closed; }

  /**
   * A world that shares the DOM but not the page's globals. Page.createIsolatedWorld
   * hands back the context id directly, so this needs no Runtime.enable — which
   * is itself a detection vector.
   */
  async ensureWorld({ force = false } = {}) {
    if (!force && this.worldContext) return this.worldContext;
    const analyzer = getAnalyzer();
    if (!analyzer) throw new Error('Analyzer unavailable for this client');

    const { frameTree } = await this.conn.send('Page.getFrameTree', {}, this.sessionId);
    const { executionContextId } = await this.conn.send('Page.createIsolatedWorld', {
      frameId: frameTree.frame.id, worldName: this.worldName, grantUniveralAccess: true,
    }, this.sessionId);
    this.worldContext = executionContextId;

    await this.conn.send('Runtime.evaluate', {
      // A recording that started on the previous page keeps going on this one.
      expression: analyzer.replace('__OYA_ATTR__', this.tagAttr).replace('__OYA_RECORD__', String(!!this.recording)),
      contextId: executionContextId, returnByValue: true,
    }, this.sessionId);
    return executionContextId;
  }

  /** Everything the analyzer needs runs here, never in the page's own world. */
  async evaluate(expression, { awaitPromise = true, retry = true } = {}) {
    const contextId = await this.ensureWorld();
    let res;
    try {
      res = await this.conn.send('Runtime.evaluate', {
        expression, contextId, returnByValue: true, awaitPromise, userGesture: true,
      }, this.sessionId);
    } catch (err) {
      // A navigation destroys the world; rebuild it once rather than failing
      // the command the user actually asked for.
      if (retry && /context|Cannot find/i.test(err.message || '')) {
        await this.ensureWorld({ force: true });
        return this.evaluate(expression, { awaitPromise, retry: false });
      }
      throw err;
    }
    if (res.exceptionDetails) throw new Error(res.exceptionDetails.exception?.description || 'Evaluation failed');
    return res.result?.value;
  }

  /** The world is created with the analyzer already in it. */
  async ensureAnalyzer() { await this.ensureWorld(); }

  /**
   * The page's own world. CAPTCHA and MFA handling has to reach page globals —
   * `___grecaptcha_cfg.clients[…].callback` is a function the page defined, and
   * an isolated world cannot see it — so those scripts run here. Nothing from
   * this driver is left behind in it.
   */
  async evaluateMain(expression, { awaitPromise = true } = {}) {
    const res = await this.conn.send('Runtime.evaluate', {
      expression, returnByValue: true, awaitPromise, userGesture: true,
    }, this.sessionId);
    if (res.exceptionDetails) throw new Error(res.exceptionDetails.exception?.description || 'Evaluation failed');
    return res.result?.value;
  }

  async mouse(type, x, y, button = 'left', clickCount = 1) {
    await this.conn.send('Input.dispatchMouseEvent', { type, x, y, button, clickCount }, this.sessionId);
  }

  async clickAt(x, y) {
    await this.mouse('mousePressed', x, y);
    await this.mouse('mouseReleased', x, y);
  }

  async locate(selector) {
    await this.ensureAnalyzer();
    const found = await this.evaluate(FIND_ELEMENT_JS(selector));
    if (!found?.ok) throw new Error(found?.error || 'Element not found');
    return found.data;
  }

  async viewport() {
    return (await this.evaluate('({width: innerWidth, height: innerHeight})')) || { width: 1280, height: 800 };
  }

  /**
   * Same action vocabulary as the Oya client, so callers never branch on client
   * type. The two grew apart — the Oya client speaks `press_key`, `list_tabs`,
   * `open_tab` and `scroll {direction}`, this driver grew hyphenated names —
   * so both spellings are accepted and normalised here rather than in every
   * caller. The SDK, the agent tools and the MCP server all go through this.
   */
  async send(action, params = {}, timeoutMs = 30000) {
    if (!this.isAlive()) throw new Error('Browser not connected');
    ({ action, params } = normalise(action, params));
    const deadline = Date.now() + timeoutMs;
    const remaining = () => Math.max(1000, deadline - Date.now());

    switch (action) {
      case 'navigate': {
        if (!params.url) return { ok: false, error: 'URL required' };
        const url = /^https?:\/\//i.test(params.url) ? params.url : `https://${params.url}`;
        const loaded = this.conn.once('Page.loadEventFired', Math.min(remaining(), 60000));
        const navigated = await this.conn.send('Page.navigate', { url }, this.sessionId, remaining());
        if (navigated.errorText) throw new Error(`Navigation failed: ${navigated.errorText}`);
        await loaded;
        return { ok: true, data: await this.pageInfo() };
      }
      case 'reload':
        await this.conn.send('Page.reload', {}, this.sessionId, remaining());
        return { ok: true };
      case 'back':
      case 'forward': {
        const { currentIndex, entries } = await this.conn.send('Page.getNavigationHistory', {}, this.sessionId);
        const target = entries[currentIndex + (action === 'back' ? -1 : 1)];
        if (!target) return { ok: false, error: `Cannot go ${action}` };
        await this.conn.send('Page.navigateToHistoryEntry', { entryId: target.id }, this.sessionId);
        return { ok: true };
      }
      case 'screenshot': {
        const { data } = await this.conn.send('Page.captureScreenshot',
          { format: 'jpeg', quality: params.quality || 60 }, this.sessionId, remaining());
        return { ok: true, data: { screenshot: `data:image/jpeg;base64,${data}` } };
      }
      case 'analyze': {
        await this.ensureAnalyzer();
        const result = await this.evaluate(
          `(typeof analyzePage === 'function') ? analyzePage(${JSON.stringify(params || {})}) : { ok: false, error: 'Analyzer not loaded' }`);
        return result;
      }
      case 'read_page':
        return { ok: true, data: await this.pageInfo() };
      /**
       * Recording what a person does in the live view. Every answer is the whole
       * buffer, not a delta, so a poll that never arrives loses nothing.
       */
      case 'record': {
        await this.ensureAnalyzer();
        if (params.mode === 'start') {
          this.recording = true;
          this.recorded = [];
          this.recordedSecrets = new Set();
          // Replay has to start on the page the person started from.
          const url = await this.evaluate('location.href').catch(() => null);
          if (/^https?:\/\//i.test(url || '')) this.recorded.push({ action: 'navigate', url, start: true, t: Date.now() });
          await this.evaluate('__acRecordStart()');
        } else if (params.mode === 'stop') {
          this.recording = false;
          await this.evaluate('__acRecordStop()').catch(() => {});
        }
        const out = await this.evaluate(`__acRecordDrain(${params.mode === 'stop' ? 'true' : 'false'})`).catch(() => null);
        for (const step of out?.steps || []) this.recorded.push(step);
        for (const name of out?.secrets || []) this.recordedSecrets.add(name);
        return { ok: true, data: { recording: !!this.recording, steps: this.recorded || [], secrets: [...(this.recordedSecrets || [])] } };
      }
      case 'click': {
        if (!params.element_id) return { ok: false, error: 'element_id required' };
        const { x, y } = await this.locate(elementSelector(params.element_id));
        await this.clickAt(x, y);
        return { ok: true, data: { clicked: true, url: await this.evaluate('location.href') } };
      }
      case 'click-coords':
        await this.clickAt(Number(params.x) || 0, Number(params.y) || 0);
        return { ok: true, data: { clicked: true } };
      // The pointer/keyboard vocabulary the Oya client has always had, so a
      // dashboard driving a browser never has to ask which kind it is.
      case 'mouse_move':
        await this.mouse('mouseMoved', Number(params.x) || 0, Number(params.y) || 0);
        return { ok: true };
      case 'double_click': {
        const { x, y } = params.selector || params.element_id
          ? await this.locate(params.selector || elementSelector(params.element_id))
          : { x: Number(params.x) || 0, y: Number(params.y) || 0 };
        await this.mouse('mousePressed', x, y, 'left', 1);
        await this.mouse('mouseReleased', x, y, 'left', 1);
        await this.mouse('mousePressed', x, y, 'left', 2);
        await this.mouse('mouseReleased', x, y, 'left', 2);
        return { ok: true };
      }
      case 'drag': {
        const fx = Number(params.from_x) || 0, fy = Number(params.from_y) || 0;
        const tx = Number(params.to_x) || 0, ty = Number(params.to_y) || 0;
        await this.mouse('mouseMoved', fx, fy);
        await this.mouse('mousePressed', fx, fy);
        // A few intermediate moves, or drag handlers that watch for movement
        // thresholds never fire.
        for (let i = 1; i <= 4; i++) {
          await this.mouse('mouseMoved', fx + (tx - fx) * (i / 4), fy + (ty - fy) * (i / 4));
        }
        await this.mouse('mouseReleased', tx, ty);
        return { ok: true };
      }
      case 'keyboard_type':
        // Into whatever is focused, like a person typing.
        await this.conn.send('Input.insertText', { text: String(params.text || '') }, this.sessionId);
        return { ok: true };
      case 'hover': {
        const { x, y } = params.element_id
          ? await this.locate(elementSelector(params.element_id))
          : { x: Number(params.x) || 0, y: Number(params.y) || 0 };
        await this.mouse('mouseMoved', x, y);
        return { ok: true };
      }
      case 'type': {
        if (params.element_id != null || params.selector) {
          const selector = params.selector || elementSelector(params.element_id);
          const { x, y } = await this.locate(selector);
          await this.clickAt(x, y);
          await this.evaluate(`(() => { const el = (window.__acFindElement || ((s) => document.querySelector(s)))(${JSON.stringify(selector)}); if (el?.isContentEditable) { const range = document.createRange(); range.selectNodeContents(el); const selection = getSelection(); selection.removeAllRanges(); selection.addRange(range); } else el?.select?.(); })()`);
        }
        await this.conn.send('Input.insertText', { text: String(params.text ?? '') }, this.sessionId);
        return { ok: true };
      }
      case 'press-key': {
        const spec = KEY_CODES[params.key];
        if (!spec) {
          // A single printable character is a keypress too; the table only
          // lists the keys that have no text of their own.
          const key = String(params.key ?? '');
          if ([...key].length === 1) {
            await this.conn.send('Input.insertText', { text: key }, this.sessionId);
            return { ok: true };
          }
          return { ok: false, error: `Unsupported key: ${params.key}` };
        }
        await this.conn.send('Input.dispatchKeyEvent', { type: 'keyDown', ...spec }, this.sessionId);
        if (spec.text) await this.conn.send('Input.dispatchKeyEvent', { type: 'char', ...spec }, this.sessionId);
        await this.conn.send('Input.dispatchKeyEvent', { type: 'keyUp', ...spec }, this.sessionId);
        return { ok: true };
      }
      case 'scroll-top':
      case 'scroll-bottom': {
        await this.evaluate(`window.scrollTo({ top: ${action === 'scroll-top' ? '0' : 'document.body.scrollHeight'} })`);
        return { ok: true };
      }
      case 'scroll-up':
      case 'scroll-down': {
        const { height } = await this.viewport();
        const delta = (params.amount || height * 0.8) * (action === 'scroll-up' ? -1 : 1);
        await this.conn.send('Input.dispatchMouseEvent',
          { type: 'mouseWheel', x: Number.isFinite(params.x) ? params.x : 10, y: Number.isFinite(params.y) ? params.y : 10, deltaX: 0, deltaY: delta }, this.sessionId);
        return { ok: true };
      }
      case 'select': {
        if (!params.element_id) return { ok: false, error: 'element_id required' };
        await this.ensureAnalyzer();
        const ok = await this.evaluate(`(() => {
          const f = window.__acFindElement || ((s) => document.querySelector(s));
          const el = f(${JSON.stringify(elementSelector(params.element_id))});
          if (!el) return false;
          el.value = ${JSON.stringify(String(params.value ?? ''))};
          el.dispatchEvent(new Event('change', { bubbles: true }));
          return true;
        })()`);
        return ok ? { ok: true } : { ok: false, error: 'Element not found' };
      }
      case 'wait': {
        const until = Date.now() + Math.min(params.timeout || 10000, remaining());
        await this.ensureAnalyzer();
        while (Date.now() < until) {
          // Element ids go through the analyzer; anything else is CSS. __acFindElement
          // reads the first number in any string, so CSS must never reach it.
          const found = await this.evaluate(`((s) => !!(/^\\d+$|^\\[data-[\\w-]+="\\d+"\\]$/.test(s) && window.__acFindElement
            ? window.__acFindElement(s) : document.querySelector(s)))(${JSON.stringify(String(params.selector || ''))})`).catch(() => false);
          if (found) return { ok: true, data: { found: true } };
          await new Promise((r) => setTimeout(r, 250));
        }
        return { ok: false, error: 'Timeout' };
      }
      case 'list-tabs': {
        const { targetInfos = [] } = await this.conn.send('Target.getTargets');
        return { ok: true, data: { tabs: targetInfos.filter((t) => t.type === 'page')
          .map((t) => ({ id: t.targetId, url: t.url, title: t.title, active: t.targetId === this.targetId })) } };
      }
      case 'new-tab': {
        const { targetId } = await this.conn.send('Target.createTarget', { url: params.url || 'about:blank' });
        await this.attach(targetId);
        return { ok: true, data: { id: targetId, tab_id: targetId } };
      }
      case 'switch-tab': {
        if (!params.id) return { ok: false, error: 'tab id required' };
        await this.attach(params.id);
        return { ok: true, data: { id: params.id } };
      }
      case 'close-tab': {
        const id = params.id || this.targetId;
        await this.conn.send('Target.closeTarget', { targetId: id });
        if (id === this.targetId) {
          const { targetInfos = [] } = await this.conn.send('Target.getTargets');
          // closeTarget can return before the tab leaves the list; never re-attach to it.
          const next = targetInfos.find((t) => t.type === 'page' && t.targetId !== id);
          if (next) await this.attach(next.targetId);
        }
        return { ok: true };
      }
      // Server-internal only: challenge handling needs to run its own scripts.
      // Not reachable from the public command API, which is why arbitrary
      // evaluate was removed from that surface.
      case 'evaluate_raw':
        // Main world on purpose: this is the channel challenge handling uses.
        return { ok: true, data: { result: await this.evaluateMain(String(params.expression || '')) } };
      case 'cookies':
        return { ok: true, data: (await this.conn.send('Network.getAllCookies', {}, this.sessionId)) };
      default:
        return { ok: false, error: `Unsupported action for a CDP browser: ${action}` };
    }
  }

  /**
   * Push frames the same way the Oya client does, so /live/:id is identical
   * for both client types. CDP screencasts natively — no polling loop.
   */
  async startScreencast(onFrame, { quality = 40, maxWidth = 1280, everyNthFrame = 2 } = {}) {
    if (this.screencasting) return;
    this.screencasting = true;
    let lastFrameAt = 0;
    this.offScreencast = this.conn.on('Page.screencastFrame', async (params, sessionId) => {
      if (sessionId && sessionId !== this.sessionId) return;
      lastFrameAt = Date.now();
      try { onFrame(`data:image/jpeg;base64,${params.data}`); } catch {}
      // Must ack or Chrome stops sending.
      this.conn.send('Page.screencastFrameAck', { sessionId: params.sessionId }, this.sessionId).catch(() => {});
    });
    await this.conn.send('Page.startScreencast',
      { format: 'jpeg', quality, maxWidth, everyNthFrame }, this.sessionId);

    // Chrome only screencasts on repaint. A page that is sitting still sends
    // nothing — so the live view of an idle browser would be blank forever,
    // which reads as "broken", not "idle". Fill the gaps with a screenshot
    // about once a second; the screencast takes over the moment anything moves.
    let busy = false;
    this.screencastFill = setInterval(async () => {
      if (busy || !this.screencasting || Date.now() - lastFrameAt < 1200) return;
      busy = true;
      try {
        const { data } = await this.conn.send('Page.captureScreenshot',
          { format: 'jpeg', quality, optimizeForSpeed: true }, this.sessionId, 5000);
        if (this.screencasting) onFrame(`data:image/jpeg;base64,${data}`);
      } catch { /* the next tick tries again */ }
      finally { busy = false; }
    }, 1000);
  }

  async stopScreencast() {
    if (!this.screencasting) return;
    this.screencasting = false;
    clearInterval(this.screencastFill);
    this.offScreencast?.();
    await this.conn.send('Page.stopScreencast', {}, this.sessionId).catch(() => {});
  }

  async pageInfo() {
    const info = await this.evaluate('({ url: location.href, title: document.title })').catch(() => null);
    return info || { url: '', title: '' };
  }

  close() { clearInterval(this.screencastFill); this.conn?.close(); }
}
