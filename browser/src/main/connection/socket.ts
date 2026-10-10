/**
 * The control socket: this browser's one connection to the server. It
 * authenticates, keeps itself alive, reconnects with backoff, and hands each
 * server message, in order, to the message map.
 */
import WebSocket from 'ws';
import { takeProxyBytes } from '../../anonymity/proxy.ts';
import type { AppServices } from '../app/services.ts';
import { OYA_ACTIONS } from '../actions/vocabulary.ts';
import { ServerMessages, type ServerMessage, type ServerMessageDeps } from './server-messages.ts';
import * as constants from './constants.ts';
import { ConnectionMessages } from './message-queue.ts';

const { CloseCode } = constants;

/** What the socket uses: the message map's services, the window for its version, and the CDP port it offers. */
type Deps = ServerMessageDeps & Pick<AppServices, 'electron' | 'cdpPort' | 'nativeBrowsing'>;

/** The calls made on a socket: the `ws` client's, or a test's stand-in. */
export interface SocketLike {
  /** CONNECTING, OPEN, CLOSING or CLOSED, as the class numbers them. */
  readyState: number;
  /** Bytes queued but not yet sent; the live view skips frames while it grows. */
  bufferedAmount: number;
  /** Sends one frame; throws when the socket is gone. */
  send(data: string): void;
  /** Closes with a code and a reason. */
  close(code?: number, reason?: string): void;
  /** Listens for open, message, close or error. */
  on(event: string, listener: (...args: never[]) => void): unknown;
  /** Drops every listener, so a close is not heard. */
  removeAllListeners(): unknown;
}

/** A socket class: opens a socket to a URL, and names the OPEN state. */
export interface SocketClass {
  /** Opens a socket to `url`. */
  new (url: string): SocketLike;
  /** The readyState of an open socket. */
  readonly OPEN: number;
}

/** Replaceable for tests: the socket class, and where proxy byte counts come from. */
export interface SocketSeams {
  /** The WebSocket class. */
  WebSocketImpl?: SocketClass;
  /** Residential proxy bytes since the last call. */
  takeBytes?: () => number;
}

/** The config values the auth message reads. */
export interface AuthConfig {
  /** The project key. */
  apiKey?: string;
  /** The name the person gave this browser. */
  browserName?: string;
  /** The persona to run as. */
  persona?: string;
  /** Where this browser runs, when the deployment says. */
  provider?: string;
}

/** A fresh browser id: `oya-` and 16 hex digits. */
export function randomId(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(constants.BROWSER_ID_BYTES));
  return 'oya-' + Array.from(bytes, (b) => b.toString(constants.HEX).padStart(constants.HEX_BYTE_DIGITS, '0')).join('');
}

/** How long to wait before reconnect attempt `attempts`: exponential, capped, jittered. */
export function reconnectDelay(attempts: number): number {
  const backoff = constants.RECONNECT_BASE_MS * Math.pow(constants.RECONNECT_GROWTH, attempts);
  return Math.min(backoff, constants.RECONNECT_MAX_MS) + Math.random() * constants.RECONNECT_JITTER_MS;
}

/** What the dev log shows for one incoming message; pings and pongs are not shown. */
function logIncoming(shell: Deps['shell'], msg: ServerMessage): void {
  if (msg.type === 'ping' || msg.type === 'pong') return;
  if (msg.type !== 'cmd') return shell.devLog('in', msg.type, msg);
  const params = msg.params && Object.keys(msg.params).length ? msg.params : '(none)';
  shell.devLog('in', `cmd: ${String(msg.action)}`, {
    id: msg.id?.slice(0, constants.ID_PREVIEW_CHARS),
    action: msg.action,
    params,
  });
}

/** Close codes after which reconnecting is pointless, and what the log says. */
const FINAL_CLOSES: Record<number, string> = {
  [CloseCode.REPLACED]: '[oya] Connection replaced by new session, not reconnecting',
  [CloseCode.AUTH_TIMEOUT]: '[oya] Auth failure, not reconnecting',
  [CloseCode.REJECTED]: '[oya] Auth failure, not reconnecting',
};

/** What parseServerMessage returns for a frame that is not JSON. */
const UNPARSEABLE = Symbol('unparseable');

/** A server frame as JSON, or UNPARSEABLE. The server is trusted to send its message shapes. */
function parseServerMessage(raw: { toString(): string }): ServerMessage | typeof UNPARSEABLE {
  try {
    return JSON.parse(raw.toString()) as ServerMessage;
  } catch {
    return UNPARSEABLE;
  }
}

/** This machine's platform as navigator.platform spells it. */
const HOST_PLATFORMS: Record<string, string> = { darwin: 'MacIntel', win32: 'Win32', linux: 'Linux x86_64' };

/**
 * Who this browser is and what it offers. `host_platform` lets the server give a
 * key's first default persona this machine's kind of device: a random one made
 * a Mac present as Linux, which sites read as an inconsistent fingerprint.
 */
export function authMessage(config: AuthConfig, id: string | null, native: number | boolean, version: string) {
  const provider = config.provider || (process.env.OYA_DOCKER ? 'oya-selfhosted' : 'oya-desktop');
  const host_platform = Object.hasOwn(HOST_PLATFORMS, process.platform) ? HOST_PLATFORMS[process.platform] : undefined;
  const who = { api_key: config.apiKey, browser_id: id, browser_name: config.browserName, host_platform };
  // So the server can count which versions run, and see an update land.
  const identity = { type: 'auth', ...who, app_version: version };
  // The server may relay CDP to our front door over this socket, and checks each command against what we do.
  const cdp = native === true;
  const offer = { provider, enrollment_token: process.env.OYA_ENROLLMENT_TOKEN, cdp, actions: OYA_ACTIONS };
  return { ...identity, persona: config.persona, ...offer };
}

/** Report setup failure only for the current connection selected by the ordered message queue. */
function messageSetupFailed(owner: ControlSocket, socket: SocketLike, msg: ServerMessage, err: unknown): void {
  console.error(`[oya] Could not handle "${msg.type}" from the server:`, (err as Error | undefined)?.stack || err);
  owner.connectionFailure = 'Session setup failed. Restart Oya to retry; if it persists, contact support.';
  socket.close(CloseCode.REJECTED, 'Session setup failed');
}

/** Share one guarded message stream across all connection generations. */
function orderedConnectionMessages(owner: ControlSocket, deps: Deps): ConnectionMessages {
  return new ConnectionMessages({
    current: () => owner.ws,
    open: () => owner.isOpen(),
    messages: new ServerMessages(deps),
    failed: (socket, msg, error) => messageSetupFailed(owner, socket, msg, error),
  });
}

/** The connection, its heartbeat and its reconnects. */
export class ControlSocket {
  /** The open (or opening) socket; the live view reads it to pace frames. */
  ws: SocketLike | null = null;
  /** True once the server has accepted our auth. */
  ready = false;
  /** Safe terminal failure shown until the next explicit connection attempt. */
  connectionFailure?: string;
  /** This browser's id, kept across reconnects. */
  browserId: string | null = null;
  /** The pending reconnect. */
  reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  /** Reconnects tried since the last success. */
  reconnectAttempts = 0;
  /** The heartbeat. */
  private pingInterval: ReturnType<typeof setInterval> | undefined = undefined;
  /** Pings sent since the server last answered. */
  private missedPongs = 0;
  /** Proxy bytes a failed send could not report yet. */
  proxyBytesUnsent = 0;
  /** The main-process services. */
  private readonly deps: Deps;
  /** Ordered routing and connection-scoped asynchronous publication. */
  private readonly messages: ConnectionMessages;
  /** The WebSocket class. */
  private readonly WebSocket: SocketClass;
  /** Where residential proxy byte counts come from. */
  private readonly takeProxyBytes: () => number;

  /** `deps` is the main-process services (see src/main/main.ts); the seams are for tests. */
  constructor(deps: Deps, { WebSocketImpl = WebSocket, takeBytes = takeProxyBytes }: SocketSeams = {}) {
    this.deps = deps;
    this.messages = orderedConnectionMessages(this, deps);
    this.WebSocket = WebSocketImpl;
    this.takeProxyBytes = takeBytes;
  }

  /** Whether the socket is open. */
  isOpen(): boolean {
    return this.ws?.readyState === this.WebSocket.OPEN;
  }

  /**
   * Send if the socket is up, otherwise drop it. A browser loses its connection
   * for ordinary reasons, sleep, wifi, a server rollout, and a send that
   * throws from inside a message handler used to take the whole session down
   * with it rather than waiting for the reconnect that was already scheduled.
   */
  send(payload: unknown): boolean {
    const ws = this.ws;
    if (!ws || !this.isOpen() || !this.messages.maySend(ws)) return false;
    try {
      ws.send(JSON.stringify(payload));
      return true;
    } catch {
      return false;
    }
  }

  /** Connects, when there is a key (or auto-connect is on). */
  connect(): void {
    if (!this.deps.config.values.apiKey && process.env.OYA_AUTO_CONNECT !== 'true') return;
    if (this.ws) this.disconnect();
    beginConnection(this);
    try {
      this.ws = this.open();
    } catch {
      this.scheduleReconnect();
    }
  }

  /** A new socket with its handlers; messages are handled one at a time, in order. */
  private open(): SocketLike {
    const socket = new this.WebSocket(this.deps.config.values.serverUrl);
    socket.on('open', () => this.authenticate(socket));
    socket.on('message', (raw: Buffer) => this.enqueue(socket, raw));
    socket.on('close', (code: number) => {
      if (this.ws === socket) this.closed(code);
    });
    socket.on('error', () => {});
    return socket;
  }

  /** Queues one message behind the last; a failed one ends a session that could not be set up. */
  private enqueue(socket: SocketLike, raw: Buffer): void {
    const msg = parseServerMessage(raw);
    if (msg === UNPARSEABLE) return;
    logIncoming(this.deps.shell, msg);
    this.messages.enqueue(socket, msg);
  }

  /** The auth message. */
  private authenticate(socket: SocketLike): void {
    if (this.ws !== socket) return;
    const config = this.deps.config.values;
    this.deps.shell.devLog('out', 'auth', { browser_id: this.browserId, browser_name: config.browserName });
    const version = this.deps.electron.app.getVersion();
    const message = authMessage(config, this.browserId, !!this.deps.nativeBrowsing, version);
    socket.send(JSON.stringify({ ...message, ...(this.deps.nativeBrowsing ? { profile_sync: true } : {}) }));
  }

  /** The socket closed: go offline, and reconnect unless the server said not to. */
  private closed(code: number): void {
    endConnection(this, code);
    this.deps.control.disconnect();
    this.deps.relay.closeCdpRelays();
    clearInterval(this.pingInterval);
    this.sendStatus();
    // Don't reconnect on fatal/intentional close codes
    if (Object.hasOwn(FINAL_CLOSES, code)) return console.log(FINAL_CLOSES[code]);
    this.scheduleReconnect();
  }

  /** Closes on purpose: no reconnect, and everything tied to the session stops. */
  disconnect(): void {
    this.deps.relay.closeCdpRelays();
    this.deps.cookies.flushCookieChanges();
    this.deps.stream.stopStream();
    this.resetTimers();
    this.dropSocket();
    this.ready = false;
    this.deps.control.disconnect();
    this.sendStatus();
  }

  /** Stops the heartbeat and any reconnect, and forgets the backoff. */
  private resetTimers(): void {
    this.connectionFailure = undefined;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    clearInterval(this.pingInterval);
    this.reconnectTimer = null;
    this.reconnectAttempts = 0;
    this.missedPongs = 0;
  }

  /** Closes the socket without hearing about it. */
  private dropSocket(): void {
    if (!this.ws) return;
    this.ws.removeAllListeners();
    try {
      this.ws.close();
    } catch {}
    this.ws = null;
  }

  /** Reconnects later, once. */
  private scheduleReconnect(): void {
    if (this.reconnectTimer) return;
    this.reconnectAttempts++;
    const reconnect = (): void => {
      this.reconnectTimer = null;
      this.connect();
    };
    this.reconnectTimer = setTimeout(reconnect, reconnectDelay(this.reconnectAttempts));
  }

  /** Tells the shell whether we are connected, and as whom. */
  sendStatus(): void {
    const profileName = this.deps.config.values.profileName || 'Default';
    this.deps.shell.send('ws-status', connectionStatus(this, profileName));
  }

  /** Starts the heartbeat. */
  startPingLoop(): void {
    clearInterval(this.pingInterval);
    this.missedPongs = 0;
    this.pingInterval = setInterval(() => this.beat(), constants.PING_INTERVAL_MS);
  }

  /** One heartbeat: ping, report proxy bytes, or give up on a silent server. */
  private beat(): void {
    this.missedPongs++;
    if (this.missedPongs > constants.MAX_MISSED_PONGS) return this.giveUp();
    this.send({ type: 'ping' });
    this.reportProxyBytes();
  }

  /** The server stopped answering: close, and let the close handler reconnect. */
  private giveUp(): void {
    clearInterval(this.pingInterval);
    if (!this.ws) return;
    try {
      this.ws.close();
    } catch {}
  }

  /** Residential proxy traffic since the last beat; kept for the next one if the send fails. */
  private reportProxyBytes(): void {
    const proxyBytes = this.takeProxyBytes();
    if (proxyBytes && !this.send({ type: 'proxy_bytes', bytes: proxyBytes })) this.proxyBytesUnsent += proxyBytes;
    else if (this.proxyBytesUnsent && this.send({ type: 'proxy_bytes', bytes: this.proxyBytesUnsent }))
      this.proxyBytesUnsent = 0;
  }

  /** The server answered: the connection is alive. */
  heard(): void {
    this.missedPongs = 0;
  }
}

/** Terminal close messages are fixed application text, never raw server reasons or credential-bearing errors. */
function terminalFailure(code: number): string | undefined {
  const reasons: Record<number, string> = {
    [CloseCode.REPLACED]: 'This connection was replaced. Restart Oya to reconnect.',
    [CloseCode.AUTH_TIMEOUT]: 'Sign-in timed out. Restart Oya to retry.',
    [CloseCode.REJECTED]: 'Sign-in was rejected. Check your account or API key in connection settings.',
  };
  return Object.hasOwn(reasons, code) ? reasons[code] : undefined;
}
/** Omit absent failures to preserve the ordinary status wire shape. */
export function failureStatus(socket: Pick<ControlSocket, 'connectionFailure'>): {
  /** Safe terminal reason. */ failure?: string;
} {
  return socket.connectionFailure ? { failure: socket.connectionFailure } : {};
}

/** A deliberate retry resets the terminal diagnostic and retains the browser identity. */
function beginConnection(owner: ControlSocket): void {
  owner.connectionFailure = undefined;
  owner.browserId ||= randomId();
  owner.sendStatus();
}
/** Retain a safe terminal diagnostic before publishing the disconnected state. */
function endConnection(owner: ControlSocket, code: number): void {
  owner.ws = null;
  owner.connectionFailure ||= terminalFailure(code);
  owner.ready = false;
}
/** Both initial IPC reads and live status events carry the same terminal failure. */
export function connectionStatus(
  socket: Pick<ControlSocket, 'ready' | 'browserId' | 'connectionFailure'>,
  profileName?: string,
) {
  return { connected: socket.ready, browserId: socket.browserId, profileName, ...failureStatus(socket) };
}
