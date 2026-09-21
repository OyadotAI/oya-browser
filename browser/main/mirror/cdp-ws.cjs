/**
 * A tiny Chrome DevTools Protocol client over a WebSocket, for talking to a
 * browser we launched on its own debugging port (not an Electron view, which
 * main/cdp.cjs already covers). Enough to read cookies, the real device, and a
 * page's localStorage, then leave.
 */
const WebSocket = require('ws');
const { CDP_COMMAND_TIMEOUT_MS } = require('./constants.cjs');

/** The browser-level WebSocket URL the debugging port advertises. */
async function debuggerUrl(port) {
  const res = await fetch(`http://127.0.0.1:${port}/json/version`);
  return (await res.json()).webSocketDebuggerUrl;
}

/** One connection to a launched browser; commands are id-correlated, events fan out to listeners. */
class CdpWs {
  /** `port` is the browser's remote-debugging port. */
  constructor(port) {
    this.port = port;
    this.ws = null;
    this.nextId = 1;
    this.pending = new Map();
    this.listeners = new Map();
  }

  /** Opens the socket to the browser target and resolves once it is ready. */
  async connect() {
    const url = await debuggerUrl(this.port);
    this.ws = new WebSocket(url, { perMessageDeflate: false });
    this.ws.on('message', (data) => this.receive(data));
    await once(this.ws, 'open');
  }

  /** Routes one incoming frame to its command reply or its event listeners. */
  receive(data) {
    const msg = JSON.parse(String(data));
    if (msg.id) return this.settle(msg);
    const fns = this.listeners.get(msg.method);
    if (fns) for (const fn of fns) fn(msg.params, msg.sessionId);
  }

  /** Resolves or rejects the command that carried this id. */
  settle(msg) {
    const p = this.pending.get(msg.id);
    if (!p) return;
    this.pending.delete(msg.id);
    if (msg.error) p.reject(new Error(msg.error.message));
    else p.resolve(msg.result);
  }

  /** Sends one command and resolves with its result; rejects on timeout or error. */
  send(method, params = {}, sessionId) {
    const id = this.nextId++;
    const frame = JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) });
    this.ws.send(frame);
    return this.awaitReply(id);
  }

  /** The promise for command `id`, armed with a timeout so a stuck browser cannot hang the import. */
  awaitReply(id) {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => this.timeout(id, reject), CDP_COMMAND_TIMEOUT_MS);
      timer.unref?.();
      this.pending.set(id, { resolve, reject: (e) => (clearTimeout(timer), reject(e)) });
    });
  }

  /** Drops a command that never answered in time. */
  timeout(id, reject) {
    this.pending.delete(id);
    reject(new Error('CDP command timed out'));
  }

  /** Registers `fn` for a CDP event; several may listen to one event. */
  on(method, fn) {
    if (!this.listeners.has(method)) this.listeners.set(method, []);
    this.listeners.get(method).push(fn);
  }

  /** Closes the socket; safe to call more than once. */
  close() {
    try {
      this.ws?.close();
    } catch {}
  }
}

/** Resolves on the emitter's next `event`, or rejects on `error`. */
function once(emitter, event) {
  return new Promise((resolve, reject) => {
    emitter.once(event, resolve);
    emitter.once('error', reject);
  });
}

module.exports = { CdpWs };
