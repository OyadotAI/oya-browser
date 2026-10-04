/**
 * A tiny Chrome DevTools Protocol client over a WebSocket, for talking to a
 * browser we launched on its own debugging port (not an Electron view, which
 * src/main/cdp/cdp.ts already covers). Enough to read cookies, the real
 * device, and a page's localStorage, then leave.
 */
import WebSocket from 'ws';
import { CDP_COMMAND_TIMEOUT_MS } from './constants.ts';

/** What the debugging port's /json/version answers. */
interface VersionInfo {
  /** The browser target's WebSocket URL. */
  webSocketDebuggerUrl: string;
}

/** Receives a CDP event's params and the session it came from. */
export type CdpListener = (params: unknown, sessionId?: string) => void;

/** One frame from the browser: a command reply (with `id`) or an event (with `method`). */
interface Frame {
  /** The command this replies to. */
  id?: number;
  /** The event's name. */
  method?: string;
  /** The event's params. */
  params?: unknown;
  /** The target session the event came from. */
  sessionId?: string;
  /** The command's result. */
  result?: unknown;
  /** Why the command failed. */
  error?: Pick<Error, 'message'>;
}

/** A command waiting for its reply. */
interface Pending {
  /** Settles the command with its result. */
  resolve: (result: unknown) => void;
  /** Fails the command. */
  reject: (e: Error) => void;
}

/** The browser-level WebSocket URL the debugging port advertises. */
async function debuggerUrl(port: number): Promise<string> {
  const res = await fetch(`http://127.0.0.1:${port}/json/version`);
  return ((await res.json()) as VersionInfo).webSocketDebuggerUrl;
}

/** One connection to a launched browser; commands are id-correlated, events fan out to listeners. */
export class CdpWs {
  /** The browser's remote-debugging port. */
  private readonly port: number;
  /** The open socket, once connected. */
  private ws: WebSocket | null = null;
  /** The id the next command gets. */
  private nextId = 1;
  /** Commands waiting for their reply, by id. */
  private readonly pending = new Map<number, Pending>();
  /** Event listeners, by CDP event name. */
  private readonly listeners = new Map<string, CdpListener[]>();

  /** `port` is the browser's remote-debugging port. */
  constructor(port: number) {
    this.port = port;
  }

  /** Opens the socket to the browser target and resolves once it is ready. */
  async connect(): Promise<void> {
    const url = await debuggerUrl(this.port);
    const ws = new WebSocket(url, { perMessageDeflate: false });
    this.ws = ws;
    ws.on('message', (data) => this.receive(data));
    await once(ws, 'open');
  }

  /** Routes one incoming frame to its command reply or its event listeners. */
  private receive(data: unknown): void {
    const msg = JSON.parse(String(data)) as Frame;
    if (msg.id) return this.settle(msg);
    const fns = msg.method ? this.listeners.get(msg.method) : undefined;
    if (fns) for (const fn of fns) fn(msg.params, msg.sessionId);
  }

  /** Resolves or rejects the command that carried this id. */
  private settle(msg: Frame): void {
    const p = msg.id === undefined ? undefined : this.pending.get(msg.id);
    if (!p) return;
    this.pending.delete(msg.id as number);
    if (msg.error) p.reject(new Error(msg.error.message));
    else p.resolve(msg.result);
  }

  /**
   * Sends one command and resolves with its result; rejects on timeout or error.
   * The send callback turns a socket that closed under us into a rejection
   * rather than a throw.
   */
  send<T = unknown>(method: string, params: object = {}, sessionId?: string): Promise<T> {
    const id = this.nextId++;
    const frame = JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) });
    const reply = this.awaitReply(id);
    (this.ws as WebSocket).send(frame, (err) => err && this.settle({ id, error: err }));
    return reply as Promise<T>;
  }

  /** The promise for command `id`, armed with a timeout so a stuck browser cannot hang the import. */
  private awaitReply(id: number): Promise<unknown> {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => this.timeout(id, reject), CDP_COMMAND_TIMEOUT_MS);
      timer.unref?.();
      this.pending.set(id, { resolve, reject: (e) => (clearTimeout(timer), reject(e)) });
    });
  }

  /** Drops a command that never answered in time. */
  private timeout(id: number, reject: (e: Error) => void): void {
    this.pending.delete(id);
    reject(new Error('CDP command timed out'));
  }

  /** Registers `fn` for a CDP event; several may listen to one event. */
  on(method: string, fn: CdpListener): void {
    const fns = this.listeners.get(method) ?? [];
    fns.push(fn);
    this.listeners.set(method, fns);
  }

  /** Closes the socket; safe to call more than once. */
  close(): void {
    try {
      this.ws?.close();
    } catch {}
  }
}

/** Resolves on the socket's next `event`, or rejects on `error`. */
function once(emitter: WebSocket, event: string): Promise<unknown> {
  return new Promise((resolve, reject) => {
    emitter.once(event, resolve);
    emitter.once('error', reject);
  });
}
