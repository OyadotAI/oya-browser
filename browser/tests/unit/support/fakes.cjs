/**
 * Test doubles for the Electron seams the main process touches: a debugger, a
 * webContents, a view, and a fake `electron` module for code that requires it
 * at load time. Hermetic: nothing here opens a window or a socket.
 */
const { EventEmitter } = require('node:events');
const path = require('node:path');

/**
 * A view's CDP debugger. `sendCommand` records each call and answers from
 * `responses` (a value, or a function of the params); `emit('message', …)`
 * delivers a protocol event the way Electron does.
 */
class FakeDebugger extends EventEmitter {
  /** `responses` maps a CDP method to its answer. */
  constructor(responses = {}) {
    super();
    /** Every command sent, in order, as `{ method, params, sessionId }`. */
    this.sent = [];
    /** Canned answers by method. */
    this.responses = responses;
    /** Whether attach() has been called. */
    this.attached = false;
  }

  /** Records the command and answers it; an Error answer rejects. */
  async sendCommand(method, params, sessionId) {
    this.sent.push({ method, params, sessionId });
    const answer = this.responses[method];
    const value = typeof answer === 'function' ? await answer(params) : answer;
    if (value instanceof Error) throw value;
    return value ?? {};
  }

  /** Marks the debugger attached. */
  attach() {
    this.attached = true;
  }

  /** Marks it detached. */
  detach() {
    this.attached = false;
  }

  /** As Electron's. */
  isAttached() {
    return this.attached;
  }

  /** Methods sent so far, for order checks. */
  methods() {
    return this.sent.map((c) => c.method);
  }

  /** Delivers a CDP event to every listener. */
  event(method, params = {}, sessionId) {
    this.emit('message', {}, method, params, sessionId);
  }
}

/** A webContents: an emitter with the few calls the main process makes. */
class FakeWebContents extends EventEmitter {
  /** `url` is what getURL() returns. */
  constructor({ url = 'about:blank', debuggerResponses } = {}) {
    super();
    /** The page's address. */
    this.url = url;
    /** Its debugger. */
    this.debugger = new FakeDebugger(debuggerResponses);
    /** Set by destroy(). */
    this.destroyed = false;
    /** Channel → payloads sent through send(). */
    this.messages = [];
    /** Chromium zoom level; 0 is actual size. */
    this.zoomLevel = 0;
  }

  /** As Electron's. */
  getZoomLevel() {
    return this.zoomLevel;
  }

  /** As Electron's. */
  setZoomLevel(level) {
    this.zoomLevel = level;
  }

  /** As Electron's. */
  getURL() {
    return this.url;
  }

  /** As Electron's. */
  isDestroyed() {
    return this.destroyed;
  }

  /** As Electron's. */
  destroy() {
    this.destroyed = true;
  }

  /** Records an IPC message to the renderer. */
  send(channel, data) {
    this.messages.push({ channel, data });
  }

  /** Payloads sent on one channel. */
  sentOn(channel) {
    return this.messages.filter((m) => m.channel === channel).map((m) => m.data);
  }
}

/** A BrowserView: webContents plus bounds. */
class FakeView {
  /** Wraps a FakeWebContents built from `options`. */
  constructor(options = {}) {
    /** The view's page. */
    this.webContents = new FakeWebContents(options);
    /** Last bounds set. */
    this.bounds = { x: 0, y: 0, width: 0, height: 0 };
  }

  /** As Electron's. */
  setBounds(bounds) {
    this.bounds = bounds;
  }

  /** As Electron's. */
  getBounds() {
    return this.bounds;
  }
}

/**
 * Puts `fake` where `require('electron')` finds it, so a module that
 * destructures Electron at load time can be required under plain Node. Returns
 * a function that restores the real entry.
 */
function installElectron(fake) {
  const id = require.resolve('electron');
  const previous = require.cache[id];
  require.cache[id] = { id, filename: id, loaded: true, exports: fake };
  return () => {
    if (previous) require.cache[id] = previous;
    else delete require.cache[id];
  };
}

/** Requires a browser module fresh, relative to the browser folder. */
function freshRequire(file) {
  const id = require.resolve(path.join(__dirname, '..', '..', '..', file));
  delete require.cache[id];
  return require(id);
}

/** Resolves once pending promise callbacks have run. */
const flush = () => new Promise((resolve) => setImmediate(resolve));

module.exports = { FakeDebugger, FakeWebContents, FakeView, installElectron, freshRequire, flush };
