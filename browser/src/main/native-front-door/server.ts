/** Authenticated loopback CDP endpoint. This server has no upstream socket or debugger dependency. */
import http, { type IncomingMessage, type ServerResponse } from 'node:http';
import { timingSafeEqual } from 'node:crypto';
import type { AddressInfo } from 'node:net';
import type { Duplex } from 'node:stream';
import { WebSocketServer, WebSocket } from 'ws';
import { NativeProtocol, ProtocolError } from './protocol.ts';
import { NATIVE_DOOR } from './constants.ts';
import type { NativeCommand, NativeDoorOptions } from './types.ts';

/** Tokens use constant-time comparison and cannot be supplied through page-readable query strings. */
function authorized(req: IncomingMessage, options: NativeDoorOptions): boolean {
  const host = req.headers.host?.split(':')[0];
  if (req.headers.origin || (host !== '127.0.0.1' && host !== 'localhost')) return false;
  const given = Buffer.from(req.headers.authorization || '');
  const wanted = Buffer.from(`Bearer ${options.token}`);
  return given.length === wanted.length && timingSafeEqual(given, wanted);
}
/** Every response is non-cacheable, and no endpoint enables CORS. */
function json(res: ServerResponse, status: number, data: unknown): void {
  res.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' });
  res.end(JSON.stringify(data));
}
/** Discovery names only this server's own sockets, never an engine endpoint or UI page. */
function discover(req: IncomingMessage, res: ServerResponse, options: NativeDoorOptions): void {
  if (!authorized(req, options)) return json(res, NATIVE_DOOR.forbidden, { error: 'Forbidden' });
  if (req.method !== 'GET') return json(res, NATIVE_DOOR.method, { error: 'Use CDP Target.createTarget' });
  try {
    const data = discoveryData(req, options);
    json(res, data === null ? NATIVE_DOOR.missing : NATIVE_DOOR.ok, data);
  } catch {
    json(res, NATIVE_DOOR.missing, { error: 'Native discovery unavailable' });
  }
}
/** Discovery URLs always point to this authenticated listener. */
function discoveryData(req: IncomingMessage, options: NativeDoorOptions): unknown {
  const base = `ws://127.0.0.1:${(req.socket.address() as AddressInfo).port}/devtools`;
  if (req.url === '/json/version')
    return { Browser: 'Oya/native', 'Protocol-Version': '1.3', webSocketDebuggerUrl: `${base}/browser` };
  if (req.url === '/json/list' || req.url === '/json')
    return options.backend
      .targets()
      .map((t) => ({ ...t, id: t.targetId, webSocketDebuggerUrl: `${base}/page/${t.targetId}` }));
  return null;
}
/** Parse only flat JSON-RPC commands; arrays and invalid id/session/params shapes are rejected. */
function command(raw: string): NativeCommand {
  const msg = JSON.parse(raw);
  if (!msg || Array.isArray(msg) || !Number.isSafeInteger(msg.id) || typeof msg.method !== 'string')
    throw new ProtocolError('Invalid command', NATIVE_DOOR.invalid);
  if (msg.sessionId !== undefined && typeof msg.sessionId !== 'string')
    throw new ProtocolError('Invalid session', NATIVE_DOOR.invalid);
  if (msg.params !== undefined && (!msg.params || typeof msg.params !== 'object' || Array.isArray(msg.params)))
    throw new ProtocolError('Invalid params', NATIVE_DOOR.invalid);
  return { ...msg, params: msg.params || {} };
}
/** Replies never outlive their requesting connection. */
function send(socket: WebSocket, data: unknown): void {
  if (socket.bufferedAmount > NATIVE_DOOR.maxPayload) return socket.terminate();
  if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(data));
}
/** Report errors explicitly; unsupported commands are never retried against a legacy backend. */
function failure(socket: WebSocket, error: unknown, msg?: NativeCommand): void {
  const code = error instanceof ProtocolError ? error.code : NATIVE_DOOR.error;
  send(socket, {
    id: msg?.id ?? null,
    ...(msg?.sessionId ? { sessionId: msg.sessionId } : {}),
    error: { code, message: errorMessage(error) },
  });
}
/** Serialize all clients so two commands cannot race native focus, navigation or element identities. */
class CommandQueue {
  /** Tail includes the entire native operation, not just its admission. */
  private tail = Promise.resolve();
  /** Bound waiting work even when a client floods requests. */
  private pending = 0;
  /** Admission and release are supplied by the existing control owner. */
  constructor(privateOptions: NativeDoorOptions) {
    this.options = privateOptions;
  }
  /** Mandatory control hooks cannot silently default to allow-all. */
  private readonly options: NativeDoorOptions;
  /** Closed sockets lose queued work; in-flight work retains its gate until it actually settles. */
  enqueue(socket: WebSocket, protocol: NativeProtocol, raw: string): void {
    let msg: NativeCommand;
    try {
      msg = command(raw);
    } catch (error) {
      return failure(socket, error);
    }
    if (resolveHeldRequest(socket, protocol, msg)) return;
    this.schedule(socket, protocol, msg);
  }
  /** Bound the shared queue before accepting another operation. */
  private schedule(socket: WebSocket, protocol: NativeProtocol, msg: NativeCommand): void {
    if (this.pending >= NATIVE_DOOR.maxQueued)
      return failure(socket, new ProtocolError('Native command queue is full'), msg);
    this.pending++;
    this.tail = this.tail
      .then(() => this.run(socket, protocol, msg))
      .catch((error) => failure(socket, error, msg))
      .finally(() => this.completed());
  }
  /** Release queue capacity only when the full native command settles. */
  private completed(): void {
    this.pending--;
  }
  /** Always recheck disconnection after waiting for authorization and always release admission. */
  private async run(socket: WebSocket, protocol: NativeProtocol, msg: NativeCommand): Promise<void> {
    if (socket.readyState !== WebSocket.OPEN) return;
    const finish = await this.options.beginCommand();
    try {
      if (socket.readyState !== WebSocket.OPEN) return;
      await reply(socket, protocol, msg);
    } finally {
      finish();
    }
  }
}
/** Wire client ownership only after authentication succeeds. */
function connected(
  socket: WebSocket,
  target: string | undefined,
  options: NativeDoorOptions,
  queue: CommandQueue,
): void {
  const protocol = socketProtocol(socket, target, options);
  watchSocket(socket, options, protocol);
  socket.on('message', (raw, binary) => receive(socket, protocol, queue, raw.toString(), binary));
}
/** Bind asynchronous failure to an explicit authenticated transport close. */
function socketProtocol(socket: WebSocket, target: string | undefined, options: NativeDoorOptions): NativeProtocol {
  return new NativeProtocol(
    options.backend,
    target,
    (event) => send(socket, event),
    (reason) => socket.close(NATIVE_DOOR.failedSocket, reason),
  );
}
/** Upgrade only exact browser/page routes; tokens, Origin and Host are checked again. */
function upgrade(req: IncomingMessage, socket: Duplex, head: Buffer, context: UpgradeContext): void {
  try {
    const { wss, options, queue } = context;
    const target = upgradeTarget(req, options);
    wss.handleUpgrade(req, socket, head, (client) => connected(client, target, options, queue));
  } catch {
    socket.end('HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n');
  }
}
/** Validate authentication before inspecting any browser targets. */
function upgradeTarget(req: IncomingMessage, options: NativeDoorOptions): string | undefined {
  if (!authorized(req, options)) throw Error('Forbidden');
  const match = /^\/devtools\/(browser|page\/([^/?]+))$/.exec(req.url || '');
  if (!match) throw Error('Unsupported endpoint');
  const target = match[2];
  if (target && !options.backend.targets().some((t) => t.targetId === target)) throw Error('Unavailable target');
  return target;
}
/** Send one flat reply after native completion, preserving session correlation. */
async function reply(socket: WebSocket, protocol: NativeProtocol, msg: NativeCommand): Promise<void> {
  const result = await protocol.dispatch(msg);
  send(socket, { id: msg.id, ...(msg.sessionId ? { sessionId: msg.sessionId } : {}), result });
}
/** Binary requests are not coerced into the supported JSON text protocol. */
function receive(socket: WebSocket, protocol: NativeProtocol, queue: CommandQueue, raw: string, binary: boolean): void {
  if (binary) return failure(socket, new ProtocolError('JSON text required'));
  queue.enqueue(socket, protocol, raw);
}
/** Start only a credentialed loopback listener; no implicit mode or stock-engine fallback exists. */
export function startNativeFrontDoor(options: NativeDoorOptions): http.Server {
  if (options.token.length < NATIVE_DOOR.tokenLength) throw new Error('A strong native front-door token is required');
  const queue = new CommandQueue(options);
  const server = http.createServer((req, res) => discover(req, res, options));
  const wss = new WebSocketServer({ noServer: true, maxPayload: NATIVE_DOOR.maxPayload, perMessageDeflate: false });
  server.on('upgrade', (req, socket, head) => upgrade(req, socket, head, { wss, options, queue }));
  server.on('close', () => dispose(wss));
  server.listen(options.port, NATIVE_DOOR.host);
  return server;
}

/** Release upgraded sockets when the listener is disposed. */
function dispose(wss: WebSocketServer): void {
  for (const client of wss.clients) client.terminate();
  wss.close();
}

/** Close bookkeeping is registered exactly once per authenticated socket. */
function watchSocket(socket: WebSocket, options: NativeDoorOptions, protocol: NativeProtocol): void {
  options.clientChanged(1);
  socket.once('close', () => {
    options.clientChanged(-1);
    protocol.dispose();
  });
  socket.on('error', () => socket.close());
}
/** Dependencies shared by upgraded connections. */
interface UpgradeContext {
  /** Local WebSocket listener, never an upstream client. */
  wss: WebSocketServer;
  /** Native capability and ownership dependencies. */
  options: NativeDoorOptions;
  /** One serial queue shared by all clients. */
  queue: CommandQueue;
}

/** Native engine exceptions can be rejected strings rather than Node Error instances. */
function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error.slice(0, NATIVE_DOOR.maxPayload);
  return 'Native command failed';
}

/** Resolving an existing native pause cannot wait behind the navigation it is unblocking. */
function resolveHeldRequest(socket: WebSocket, protocol: NativeProtocol, msg: NativeCommand): boolean {
  if (msg.method !== 'Fetch.continueRequest' && msg.method !== 'Fetch.failRequest') return false;
  void reply(socket, protocol, msg).catch((error) => failure(socket, error, msg));
  return true;
}
