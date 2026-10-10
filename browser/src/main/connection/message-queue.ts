/** Serialize control messages across socket replacements and fence stale asynchronous publication. */
import type { SocketLike } from './socket.ts';
import type { ServerMessage, ServerMessages } from './server-messages.ts';
/** Socket identity and routing remain owned by the connection lifecycle. */
export interface MessageQueueDeps {
  /** Currently selected open socket, or null after disconnection. */
  current(): SocketLike | null;
  /** Whether the selected socket is open. */
  open(): boolean;
  /** Message handlers with authentication continuation guards. */
  messages: ServerMessages;
  /** Report failures only to the exact originating connection. */
  failed(socket: SocketLike, msg: ServerMessage, error: unknown): void;
}
/** One ordered stream survives reconnects without letting old work publish on a new socket. */
export class ConnectionMessages {
  /** A failed handler never strands the following connection's messages. */
  private tail: Promise<unknown> = Promise.resolve();
  /** The exact connection whose asynchronous message is executing. */
  private delivering: SocketLike | null = null;
  /** Injectable lifecycle and handler seams. */
  private readonly deps: MessageQueueDeps;
  /** Construct once for the lifetime of the application connection. */
  constructor(deps: MessageQueueDeps) {
    this.deps = deps;
  }
  /** New messages wait for earlier identity setup even when their sockets differ. */
  enqueue(socket: SocketLike, message: ServerMessage): void {
    this.tail = this.tail
      .then(() => this.deliver(socket, message))
      .catch((error) => {
        if (this.deps.current() === socket) this.deps.failed(socket, message, error);
      });
  }
  /** An old asynchronous handler cannot leak its profile data onto the new transport. */
  maySend(socket: SocketLike): boolean {
    return !this.delivering || this.delivering === socket;
  }
  /** Closed and replaced connections cannot resume authentication or begin queued work. */
  private current(socket: SocketLike): boolean {
    return this.deps.current() === socket && this.deps.open();
  }
  /** Recheck connection identity at each authentication continuation. */
  private guard(socket: SocketLike): void {
    if (!this.current(socket)) throw Error('Control connection replaced');
  }
  /** Pin asynchronous publication and always release it before the next queued message. */
  private async deliver(socket: SocketLike, message: ServerMessage): Promise<void> {
    if (!this.current(socket)) return;
    this.delivering = socket;
    try {
      await this.deps.messages.handle(message, () => this.guard(socket));
    } finally {
      this.delivering = null;
    }
  }
}
