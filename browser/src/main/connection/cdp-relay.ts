/**
 * The server's gateway reaches our CDP front door through the control socket,
 * so a sandbox or a desktop behind NAT needs no inbound port. sid → local socket.
 */
import WebSocket from 'ws';
import type { AppServices } from '../app/services.ts';
import { MAX_CDP_PAYLOAD } from './constants.ts';

/** The front door's port, the token that proves the relay is ours, and the socket to report over. */
type Deps = Pick<AppServices, 'cdpPort' | 'relayToken' | 'socket'>;

/** What the debugging port's /json/version answers. */
interface VersionInfo {
  /** The browser target's WebSocket URL. */
  webSocketDebuggerUrl: string;
}

/** Connects to the front door's browser target, carrying the relay token. */
async function dialRelay(port: number, token: string): Promise<WebSocket> {
  const res = await fetch(`http://127.0.0.1:${port}/json/version`);
  const { webSocketDebuggerUrl } = (await res.json()) as VersionInfo;
  return new WebSocket(`ws://127.0.0.1:${port}${new URL(webSocketDebuggerUrl).pathname}`, {
    headers: { 'X-Oya-Relay': token },
    perMessageDeflate: false,
    maxPayload: MAX_CDP_PAYLOAD,
  });
}

/** CDP sessions the server relays to our front door, one local socket each. */
export class CdpRelay {
  /** Open relayed sockets by session id; the server's frames are written to them. */
  readonly cdpRelays = new Map<string | undefined, WebSocket>();
  /** The port, the token and the control socket. */
  private readonly deps: Deps;

  /** `deps` names the CDP front door, the relay token and the control socket. */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Opens one relayed CDP socket onto the front door's browser endpoint. */
  async openCdpRelay(sid: string | undefined): Promise<void> {
    const fail = (error: string): void => this.fail(sid, error);
    if (!this.deps.cdpPort) return fail('CDP is off in this browser. Start it with OYA_REMOTE_DEBUGGING_PORT set.');
    try {
      this.wire(sid, await dialRelay(this.deps.cdpPort, this.deps.relayToken), fail);
    } catch (e) {
      fail((e as Error).message);
    }
  }

  /** Closes every relayed socket. */
  closeCdpRelays(): void {
    for (const sock of this.cdpRelays.values())
      try {
        sock.close();
      } catch {}
    this.cdpRelays.clear();
  }

  /** Drops `sid` and tells the server its relay closed, with the reason. */
  private fail(sid: string | undefined, error: string): void {
    this.cdpRelays.delete(sid);
    this.deps.socket.send({ type: 'cdp_closed', sid, error });
  }

  /** Registers the socket under `sid` and forwards its life over the control socket. */
  private wire(sid: string | undefined, sock: WebSocket, fail: (error: string) => void): void {
    const send = (payload: object): unknown => this.deps.socket.send(payload);
    this.cdpRelays.set(sid, sock);
    sock.on('open', () => send({ type: 'cdp_opened', sid }));
    sock.on('message', (data) => send({ type: 'cdp', sid, data: data.toString() }));
    sock.on('close', () => this.cdpRelays.delete(sid) && send({ type: 'cdp_closed', sid }));
    sock.on('error', (e) => this.cdpRelays.has(sid) && fail(e.message));
  }
}
