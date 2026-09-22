/**
 * Minimal CDP JSON-RPC transport over the ws dependency we already have:
 * request/response by id, events by method name.
 */
import WebSocket from 'ws';
import { CONNECT_TIMEOUT_MS, MAX_PAYLOAD_BYTES, COMMAND_TIMEOUT_MS } from './constants.ts';

/**
 * The connection itself failed: closed, refused a write, or gave no reply in
 * time. Distinct from an error the page answered with, because the outcome of
 * a command sent over a lost connection is unknown, while a page's "no" is known.
 */
export class CdpConnectionError extends Error {}

/** Minimal CDP JSON-RPC transport over the ws dependency we already have. */
export class CDPConnection {
  /** Set once the socket closes; every later send fails fast. */
  declare closed: any;
  /** Event handlers by CDP method name. */
  declare listeners: Map<any, any>;
  /** Id for the next outgoing request. */
  declare nextId: number;
  /** Requests awaiting a reply, by id, with their resolvers and timeout. */
  declare pending: Map<any, any>;
  /** The browser's CDP WebSocket URL. */
  declare url: any;
  /** The underlying socket, set by connect(). */
  declare ws: WebSocket;
  constructor(url) {
    this.url = url;
    this.nextId = 1;
    this.pending = new Map();
    this.listeners = new Map();
    this.closed = false;
  }

  /** Opens the socket; resolves with this connection, rejects on error or timeout. */
  connect(timeoutMs = CONNECT_TIMEOUT_MS) {
    return new Promise((resolve, reject) => {
      const finish = settleOnce(resolve, reject, this, () => clearTimeout(timer));
      const timer = setTimeout(() => finish(new Error('CDP connect timed out')), timeoutMs);
      this.ws = new WebSocket(this.url, { maxPayload: MAX_PAYLOAD_BYTES, handshakeTimeout: timeoutMs });
      watchSocket(this, finish);
    });
  }

  /**
   * Speaks CDP over a socket somebody else opened: a relay carried on an Oya
   * browser's control socket, or a socket a driver's endpoint dialled. Nothing
   * here dials, so a caller holds one way of reaching a browser, not two.
   */
  static over(socket: WebSocket): CDPConnection {
    const conn = new CDPConnection(socket.url ?? null);
    conn.ws = socket;
    watchTraffic(conn);
    return conn;
  }

  /** Routes a reply to its pending request, or an event to its listeners. */
  onMessage(raw) {
    let msg;
    try {
      msg = JSON.parse(raw.toString());
    } catch {
      return;
    }
    if (msg.id != null) return settleReply(this.pending, msg);
    if (msg.method) emit(this.listeners, msg);
  }

  /** Rejects every in-flight request with the same error. */
  failAll(err) {
    for (const [, p] of this.pending) {
      clearTimeout(p.timer);
      p.reject(err);
    }
    this.pending.clear();
  }

  /** Subscribes to a CDP event; returns a function that unsubscribes. */
  on(method, fn) {
    if (!this.listeners.has(method)) this.listeners.set(method, new Set());
    this.listeners.get(method).add(fn);
    return () => this.listeners.get(method)?.delete(fn);
  }

  /** Wait for one occurrence of a CDP event, or resolve null on timeout. */
  once(method, timeoutMs) {
    return new Promise((resolve) => awaitEvent(this, method, timeoutMs, resolve));
  }

  /** Sends one CDP command, optionally into a flattened target session, and resolves with its result. */
  send(method, params = {}, sessionId, timeoutMs = COMMAND_TIMEOUT_MS) {
    if (!isOpen(this)) return Promise.reject(new CdpConnectionError('CDP connection closed'));
    const id = this.nextId++;
    const message = sessionId ? { id, method, params, sessionId } : { id, method, params };
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => expire(this.pending, id, method, reject), timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      write(this, id, message, timer, reject);
    });
  }

  /** Closes the socket; later sends reject. */
  close() {
    this.closed = true;
    try {
      this.ws?.close();
    } catch {}
  }
}

/** A settle function that counts only the first outcome and runs `cleanup` with it. */
function settleOnce(resolve, reject, value, cleanup) {
  let settled = false;
  return (err?) => {
    if (settled) return;
    settled = true;
    cleanup();
    if (err) reject(err);
    else resolve(value);
  };
}

/** Wires the socket's lifecycle to the connection: open settles connect, an error fails it, and traffic flows. */
function watchSocket(conn: CDPConnection, finish) {
  conn.ws.on('open', () => finish());
  conn.ws.on('error', (e) => finish(e));
  watchTraffic(conn);
}

/** Routes the socket's traffic: messages to their requests and listeners, close and error to every request. */
function watchTraffic(conn: CDPConnection) {
  conn.ws.on('error', (e) => conn.failAll(new CdpConnectionError(e.message)));
  conn.ws.on('close', () => markClosed(conn));
  conn.ws.on('message', (raw) => conn.onMessage(raw));
}

/** The socket closed: nothing more will be answered. */
function markClosed(conn: CDPConnection) {
  conn.closed = true;
  conn.failAll(new CdpConnectionError('CDP connection closed'));
}

/** Whether a request can be written now. */
function isOpen(conn: CDPConnection) {
  return !conn.closed && conn.ws?.readyState === WebSocket.OPEN;
}

/** Resolves with the event's params the first time it fires, or null once timeoutMs passes. */
function awaitEvent(conn: CDPConnection, method, timeoutMs, resolve) {
  const done = (params) => {
    clearTimeout(timer);
    off();
    resolve(params);
  };
  const timer = setTimeout(() => done(null), timeoutMs);
  const off = conn.on(method, done);
}

/** Resolves or rejects the request a reply answers; a reply nobody waits for is dropped. */
function settleReply(pending, msg) {
  const p = pending.get(msg.id);
  if (!p) return;
  clearTimeout(p.timer);
  pending.delete(msg.id);
  if (msg.error) p.reject(new Error(msg.error.message || 'CDP error'));
  else p.resolve(msg.result);
}

/** Hands an event to each listener; one throwing listener does not stop the rest. */
function emit(listeners, msg) {
  for (const fn of listeners.get(msg.method) || []) {
    try {
      fn(msg.params, msg.sessionId);
    } catch {}
  }
}

/** Gives up on a request that got no reply in time. */
function expire(pending, id, method, reject) {
  pending.delete(id);
  reject(new CdpConnectionError(`CDP ${method} timed out`));
}

/** Writes a request; a socket that refuses it fails the request at once. */
function write(conn: CDPConnection, id, message, timer, reject) {
  try {
    conn.ws.send(JSON.stringify(message));
  } catch (e) {
    clearTimeout(timer);
    conn.pending.delete(id);
    reject(new CdpConnectionError(e.message));
  }
}
