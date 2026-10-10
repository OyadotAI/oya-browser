/** External CDP frames arriving over the authenticated control socket terminate in Oya's native adapter. */
import { NativeRelayConnection, NativeRelayQueue, type NativeBackend } from '../native-front-door/index.ts';
import { MAX_NATIVE_RELAYS } from './constants.ts';

/** Native operations and authenticated ownership supplied by the composition root. */
interface Deps {
  /** Shared target identities; each connection receives independent native resources. */
  backend: NativeBackend;
  /** Current server authentication and human/agent control state. */
  allowed(): boolean;
  /** Existing authenticated control socket. */
  socket: {
    /** Deliver a protocol response without opening another transport. */
    send(message: object): unknown;
  };
}

/** Compatibility facade retained for control-socket messages; no upstream debugging connection exists. */
export class CdpRelay {
  /** Each external session owns a native protocol namespace. */
  readonly cdpRelays = new Map<string | undefined, NativeRelayConnection>();
  /** Bounded ordering is shared across every remote session. */
  private readonly queue = new NativeRelayQueue();
  /** Native operations and live authorization, never a port or relay secret. */
  private readonly deps: Deps;
  /** Bind this control socket to its application's native capabilities. */
  constructor(deps: Deps) {
    this.deps = deps;
  }
  /** Reject duplicate or unauthenticated opens without replacing an existing session. */
  async openCdpRelay(sid: string | undefined): Promise<void> {
    if (!sid || !this.deps.allowed()) return this.reject(sid, 'Remote native control is unavailable');
    if (this.cdpRelays.has(sid)) return;
    if (this.cdpRelays.size >= MAX_NATIVE_RELAYS) return this.reject(sid, 'Native relay limit exceeded');
    this.open(sid);
  }
  /** Construction failures do not leave a live but unreported native connection. */
  private open(sid: string): void {
    try {
      this.cdpRelays.set(sid, this.connection(sid));
      this.deps.socket.send({ type: 'cdp_opened', sid });
    } catch {
      this.cdpRelays.get(sid)?.close();
      this.reject(sid, 'Native relay could not be opened');
    }
  }
  /** Native connection events are delivered only to their originating remote session. */
  private connection(sid: string): NativeRelayConnection {
    const options = {
      backend: this.deps.backend,
      allowed: () => this.deps.allowed(),
      emit: (message: object) => this.deps.socket.send({ type: 'cdp', sid, data: JSON.stringify(message) }),
      closed: (error?: string) => this.closed(sid, error),
    };
    return new NativeRelayConnection(options, this.queue);
  }
  /** Revoke every session even when one resource cleanup fails. */
  closeCdpRelays(): void {
    const connections = [...this.cdpRelays.values()];
    this.cdpRelays.clear();
    for (const connection of connections) closeConnection(connection);
  }
  /** Closing a session already removed by the server does not echo another close. */
  private closed(sid: string, error?: string): void {
    if (this.cdpRelays.delete(sid)) this.deps.socket.send({ type: 'cdp_closed', sid, ...(error ? { error } : {}) });
  }
  /** Admission failures never allocate native resources. */
  private reject(sid: string | undefined, error: string): void {
    this.deps.socket.send({ type: 'cdp_closed', sid, error });
  }
}

/** One resource failure must not strand another authenticated connection. */
function closeConnection(connection: NativeRelayConnection): void {
  try {
    connection.close();
  } catch {
    console.error('[native-relay] resource cleanup failed');
  }
}
