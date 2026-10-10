/** Terminate server-relayed external protocol frames directly in authorized native operations. */
import { nativeCommand } from './command.ts';
import { NativeProtocol, ProtocolError } from './protocol.ts';
import { NATIVE_DOOR } from './constants.ts';
import type { NativeBackend, NativeCommand } from './types.ts';

/** Authenticated control transport and current ownership, supplied by the application. */
interface RelayOptions {
  /** Native browser operations with connection-owned resources. */
  backend: NativeBackend;
  /** Recheck the server connection and live human/agent ownership. */
  allowed(): boolean;
  /** Deliver a response or event to this exact remote connection. */
  emit(message: object): void;
  /** Report terminal failure and revoke the remote connection. */
  closed(reason?: string): void;
}

/** All remote clients share bounded serial admission without acquiring the server's lease twice. */
export class NativeRelayQueue {
  /** Preserve native focus and navigation ordering across remote clients. */
  private tail = Promise.resolve();
  /** Waiting and executing work both consume capacity. */
  private pending = 0;
  /** Rejected work never reaches native operations. */
  run(operation: () => Promise<void>): Promise<void> {
    if (this.pending >= NATIVE_DOOR.maxQueued) return Promise.reject(new ProtocolError('Native command queue is full'));
    this.pending++;
    const result = this.tail.then(operation);
    this.tail = result.catch(() => {}).finally(() => this.pending--);
    return result;
  }
}

/** A remote transport owns a protocol namespace, never a connection to Chromium. */
export class NativeRelayConnection {
  /** Closed transports cannot execute queued commands or emit late replies. */
  private active = true;
  /** Native handles and observers belong only to this connection. */
  private readonly protocol: NativeProtocol;
  /** Connection-scoped authorization and transport callbacks. */
  private readonly options: RelayOptions;
  /** Shared scheduling preserves ordering across relays. */
  private readonly queue: NativeRelayQueue;
  /** The caller must authenticate before constructing a connection. */
  constructor(options: RelayOptions, queue: NativeRelayQueue) {
    this.options = options;
    this.queue = queue;
    this.protocol = new NativeProtocol(
      options.backend,
      undefined,
      (m) => this.emit(m),
      (e) => this.close(e),
    );
  }
  /** Validate bounded external input before accepting work. */
  send(raw: string): void {
    if (!this.active) return;
    let message: NativeCommand;
    try {
      message = nativeCommand(raw);
    } catch (error) {
      return this.failure(error);
    }
    void this.schedule(message).catch((error) => this.failure(error, message));
  }
  /** Held native requests must resolve while the navigation that owns them is waiting. */
  private schedule(message: NativeCommand): Promise<void> {
    const operation = () => this.execute(message);
    if (message.method === 'Fetch.continueRequest' || message.method === 'Fetch.failRequest') return operation();
    return this.queue.run(operation);
  }
  /** Recheck ownership after queueing; the server already holds its actor-specific command lease. */
  private async execute(message: NativeCommand): Promise<void> {
    if (!this.active) return;
    if (!this.options.allowed()) throw new ProtocolError('Remote native control is unavailable');
    if (message.method === 'Browser.setDownloadBehavior')
      throw new ProtocolError('Remote callers cannot select local download destinations', NATIVE_DOOR.unsupported);
    const result = await this.protocol.dispatch(message);
    if (!this.options.allowed()) throw new ProtocolError('Remote native control was revoked');
    this.emit({ id: message.id, ...(message.sessionId ? { sessionId: message.sessionId } : {}), result });
  }
  /** Suppress all native events and results after disconnect or a human takeover. */
  private emit(message: object): void {
    if (this.active && this.options.allowed()) this.options.emit(message);
  }
  /** Correlated errors remain deliverable during a takeover so callers do not wait indefinitely. */
  private failure(error: unknown, message?: NativeCommand): void {
    if (!this.active) return;
    this.options.emit({
      id: message?.id ?? null,
      ...(message?.sessionId ? { sessionId: message.sessionId } : {}),
      error: relayError(error),
    });
  }
  /** Revoke access before cleanup, including when native resource disposal fails. */
  close(reason?: string): void {
    if (!this.active) return;
    this.active = false;
    try {
      this.protocol.dispose();
    } finally {
      this.options.closed(reason);
    }
  }
}

/** Preserve protocol categories without leaking arbitrary rejection objects. */
function relayError(error: unknown) {
  return {
    code: error instanceof ProtocolError ? error.code : NATIVE_DOOR.error,
    message: error instanceof Error ? error.message : 'Native command failed',
  };
}
