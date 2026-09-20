/**
 * The control socket: this browser's one connection to the server. It
 * authenticates, keeps itself alive, reconnects with backoff, and hands each
 * server message, in order, to the message map.
 */
const crypto = require('crypto');
const WebSocket = require('ws');
const { takeProxyBytes } = require('../../anonymity/proxy');
const { handleServerMessage } = require('./server-messages.cjs');
const constants = require('./constants.cjs');

const { CloseCode } = constants;

/** A fresh browser id: `oya-` and 16 hex digits. */
function randomId() {
  const bytes = crypto.getRandomValues(new Uint8Array(constants.BROWSER_ID_BYTES));
  return 'oya-' + Array.from(bytes, (b) => b.toString(constants.HEX).padStart(constants.HEX_BYTE_DIGITS, '0')).join('');
}

/** How long to wait before reconnect attempt `attempts`: exponential, capped, jittered. */
function reconnectDelay(attempts) {
  const backoff = constants.RECONNECT_BASE_MS * Math.pow(constants.RECONNECT_GROWTH, attempts);
  return Math.min(backoff, constants.RECONNECT_MAX_MS) + Math.random() * constants.RECONNECT_JITTER_MS;
}

/** What the dev log shows for one incoming message; pings and pongs are not shown. */
function logIncoming(shell, msg) {
  if (msg.type === 'ping' || msg.type === 'pong') return;
  if (msg.type !== 'cmd') return shell.devLog('in', msg.type, msg);
  const params = msg.params && Object.keys(msg.params).length ? msg.params : '(none)';
  shell.devLog('in', `cmd: ${msg.action}`, {
    id: msg.id?.slice(0, constants.ID_PREVIEW_CHARS),
    action: msg.action,
    params,
  });
}

/** Close codes after which reconnecting is pointless, and what the log says. */
const FINAL_CLOSES = {
  [CloseCode.REPLACED]: '[oya] Connection replaced by new session — not reconnecting',
  [CloseCode.AUTH_TIMEOUT]: '[oya] Auth failure — not reconnecting',
  [CloseCode.REJECTED]: '[oya] Auth failure — not reconnecting',
};

/** What parseServerMessage returns for a frame that is not JSON. */
const UNPARSEABLE = Symbol('unparseable');

/** A server frame as JSON, or UNPARSEABLE. */
function parseServerMessage(raw) {
  try {
    return JSON.parse(raw.toString());
  } catch {
    return UNPARSEABLE;
  }
}

/** Who this browser is and what it offers. */
function authMessage(config, browserId, cdpPort) {
  const provider = config.provider || (process.env.OYA_DOCKER ? 'oya-selfhosted' : 'oya-desktop');
  const identity = { type: 'auth', api_key: config.apiKey, browser_id: browserId, browser_name: config.browserName };
  // The server may relay CDP to our front door over this socket.
  const offer = { provider, enrollment_token: process.env.OYA_ENROLLMENT_TOKEN, cdp: !!cdpPort };
  return { ...identity, persona: config.persona, ...offer };
}

/** The connection, its heartbeat and its reconnects. */
class ControlSocket {
  /** The open (or opening) socket. */
  ws = null;
  /** True once the server has accepted our auth. */
  ready = false;
  /** This browser's id, kept across reconnects. */
  browserId = null;
  /** The pending reconnect. */
  reconnectTimer = null;
  /** Reconnects tried since the last success. */
  reconnectAttempts = 0;
  /** The heartbeat. */
  pingInterval = null;
  /** Pings sent since the server last answered. */
  missedPongs = 0;
  /** Proxy bytes a failed send could not report yet. */
  proxyBytesUnsent = 0;

  /** `ctx` is the main-process context (see main.js); `WebSocketImpl` and `takeBytes` are seams for tests. */
  constructor(ctx, { WebSocketImpl = WebSocket, takeBytes = takeProxyBytes } = {}) {
    /** The main-process context. */
    this.ctx = ctx;
    /** The WebSocket class. */
    this.WebSocket = WebSocketImpl;
    /** Where residential proxy byte counts come from. */
    this.takeProxyBytes = takeBytes;
  }

  /** Whether the socket is open. */
  isOpen() {
    return this.ws?.readyState === this.WebSocket.OPEN;
  }

  /**
   * Send if the socket is up, otherwise drop it. A browser loses its connection
   * for ordinary reasons — sleep, wifi, a server rollout — and a send that
   * throws from inside a message handler used to take the whole session down
   * with it rather than waiting for the reconnect that was already scheduled.
   */
  send(payload) {
    if (!this.isOpen()) return false;
    try {
      this.ws.send(JSON.stringify(payload));
      return true;
    } catch {
      return false;
    }
  }

  /** Connects, when there is a key (or auto-connect is on). */
  connect() {
    if (!this.ctx.config.values.apiKey && process.env.OYA_AUTO_CONNECT !== 'true') return;
    if (this.ws) this.disconnect();
    this.browserId = this.browserId || randomId();
    try {
      this.ws = this.open();
    } catch {
      this.scheduleReconnect();
    }
  }

  /** A new socket with its handlers; messages are handled one at a time, in order. */
  open() {
    const socket = new this.WebSocket(this.ctx.config.values.serverUrl);
    let messageQueue = Promise.resolve();
    socket.on('open', () => this.authenticate(socket));
    socket.on('message', (raw) => (messageQueue = this.enqueue(messageQueue, socket, raw)));
    socket.on('close', (code) => this.closed(code));
    socket.on('error', () => {});
    return socket;
  }

  /** Queues one message behind the last; a failed one ends a session that could not be set up. */
  enqueue(queue, socket, raw) {
    const msg = parseServerMessage(raw);
    if (msg === UNPARSEABLE) return queue;
    logIncoming(this.ctx.shell, msg);
    return queue
      .then(() => handleServerMessage(this.ctx, msg))
      .catch(() => socket.close(CloseCode.REJECTED, 'Session setup failed'));
  }

  /** The auth message. */
  authenticate(socket) {
    const config = this.ctx.config.values;
    this.ctx.shell.devLog('out', 'auth', { browser_id: this.browserId, browser_name: config.browserName });
    socket.send(JSON.stringify(authMessage(config, this.browserId, this.ctx.cdpPort)));
  }

  /** The socket closed: go offline, and reconnect unless the server said not to. */
  closed(code) {
    this.ready = false;
    this.ctx.control.disconnect();
    this.ctx.relay.closeCdpRelays();
    clearInterval(this.pingInterval);
    this.sendStatus();
    // Don't reconnect on fatal/intentional close codes
    if (Object.hasOwn(FINAL_CLOSES, code)) return console.log(FINAL_CLOSES[code]);
    this.scheduleReconnect();
  }

  /** Closes on purpose: no reconnect, and everything tied to the session stops. */
  disconnect() {
    this.ctx.relay.closeCdpRelays();
    this.ctx.cookies.flushCookieChanges();
    this.ctx.stream.stopStream();
    this.resetTimers();
    this.dropSocket();
    this.ready = false;
    this.ctx.control.disconnect();
    this.sendStatus();
  }

  /** Stops the heartbeat and any reconnect, and forgets the backoff. */
  resetTimers() {
    clearTimeout(this.reconnectTimer);
    clearInterval(this.pingInterval);
    this.reconnectTimer = null;
    this.reconnectAttempts = 0;
    this.missedPongs = 0;
  }

  /** Closes the socket without hearing about it. */
  dropSocket() {
    if (!this.ws) return;
    this.ws.removeAllListeners();
    try {
      this.ws.close();
    } catch {}
    this.ws = null;
  }

  /** Reconnects later, once. */
  scheduleReconnect() {
    if (this.reconnectTimer) return;
    this.reconnectAttempts++;
    const reconnect = () => {
      this.reconnectTimer = null;
      this.connect();
    };
    this.reconnectTimer = setTimeout(reconnect, reconnectDelay(this.reconnectAttempts));
  }

  /** Tells the shell whether we are connected, and as whom. */
  sendStatus() {
    const profileName = this.ctx.config.values.profileName || 'Default';
    this.ctx.shell.send('ws-status', { connected: this.ready, browserId: this.browserId, profileName });
  }

  /** Starts the heartbeat. */
  startPingLoop() {
    clearInterval(this.pingInterval);
    this.missedPongs = 0;
    this.pingInterval = setInterval(() => this.beat(), constants.PING_INTERVAL_MS);
  }

  /** One heartbeat: ping, report proxy bytes, or give up on a silent server. */
  beat() {
    this.missedPongs++;
    if (this.missedPongs > constants.MAX_MISSED_PONGS) return this.giveUp();
    this.send({ type: 'ping' });
    this.reportProxyBytes();
  }

  /** The server stopped answering: close, and let the close handler reconnect. */
  giveUp() {
    clearInterval(this.pingInterval);
    if (!this.ws) return;
    try {
      this.ws.close();
    } catch {}
  }

  /** Residential proxy traffic since the last beat; kept for the next one if the send fails. */
  reportProxyBytes() {
    const proxyBytes = this.takeProxyBytes();
    if (proxyBytes && !this.send({ type: 'proxy_bytes', bytes: proxyBytes })) this.proxyBytesUnsent += proxyBytes;
    else if (this.proxyBytesUnsent && this.send({ type: 'proxy_bytes', bytes: this.proxyBytesUnsent }))
      this.proxyBytesUnsent = 0;
  }

  /** The server answered: the connection is alive. */
  heard() {
    this.missedPongs = 0;
  }
}

module.exports = { ControlSocket, randomId, reconnectDelay, authMessage };
