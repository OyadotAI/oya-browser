/**
 * Client commands on their way to the browser: one at a time and in order,
 * each holding a control-plane command slot until the browser replies (or
 * STUCK_COMMAND_MS passes), so human takeover can wait for them.
 */
import { WebSocket } from 'ws';
import { control } from '../control/service.ts';
import { CloseCode, STUCK_COMMAND_MS } from './constants.ts';

/** The parts of a session the gate reads and writes. */
export interface GatedSession {
  /** Session id. */
  id: string;
  /** The registry browser attached to, if any; commands are counted against it. */
  attachedTo?: string;
  /** Set once the session is ending. */
  closed: boolean;
  /** The connected client. */
  client: any;
  /** The browser's socket. */
  upstream: any;
  /** Bytes forwarded from the client to the browser. */
  bytesUp: number;
}

/** Forwards a session's client commands and tracks their in-flight slots. */
export class CommandGate {
  /** In-flight command slots by `sessionId:id`, released on reply or after STUCK_COMMAND_MS. */
  readonly releases = new Map();
  /** Chains client commands so they reach the browser one at a time, in order. */
  private tail: Promise<void> = Promise.resolve();
  /** The session whose commands these are. */
  declare private readonly session: GatedSession;

  /** Gates `session`'s commands. */
  constructor(session: GatedSession) {
    this.session = session;
  }

  /** Queues one client message; a refused command closes that client. */
  enqueue(client, data, isBinary) {
    this.tail = this.tail
      .then(() => this.forward(client, data, isBinary))
      .catch(() => client.close(CloseCode.POLICY_VIOLATION, 'Session access paused, revoked, or command invalid'));
  }

  /** Sends a command to the browser once it holds a slot. */
  private async forward(client, data, isBinary) {
    const s = this.session;
    if (s.closed || s.client !== client) return;
    // Revocation is enforced by the attachment validator every two seconds, not per CDP message.
    const commandKey = this.keyOf(data);
    const finish = await this.begin(commandKey);
    s.bytesUp += data.length;
    if (s.upstream.readyState !== WebSocket.OPEN) return this.abandon(commandKey, finish);
    s.upstream.send(data, { binary: isBinary });
  }

  /** The command's `sessionId:id`; a command without an id, or a duplicate, is refused. */
  private keyOf(data) {
    const command = JSON.parse(data.toString());
    if (command.id === undefined) throw new Error('CDP command ID is required');
    const commandKey = `${command.sessionId || ''}:${command.id}`;
    if (this.releases.has(commandKey)) throw new Error('Duplicate CDP command ID');
    return commandKey;
  }

  /** Takes a control-plane command slot; the returned function gives it back. */
  private async begin(commandKey) {
    const settle = await control().beginCommand(this.session.attachedTo || this.session.id);
    // A command the browser never answers must not block human takeover forever.
    const timer = setTimeout(() => this.settle(commandKey), STUCK_COMMAND_MS);
    const finish = () => {
      clearTimeout(timer);
      return settle();
    };
    this.releases.set(commandKey, finish);
    return finish;
  }

  /** The browser went away: give the slot back and refuse the command. */
  private async abandon(commandKey, finish) {
    this.releases.delete(commandKey);
    await finish();
    throw new Error('Browser disconnected');
  }

  /** Release a command's in-flight slot: on its reply, or once it has run for STUCK_COMMAND_MS. */
  settle(key) {
    const finish = this.releases.get(key);
    if (finish) {
      this.releases.delete(key);
      void finish().catch(() => {});
    }
  }

  /** A browser message: if it is a reply, its command's slot is released. */
  settleReply(data) {
    try {
      const reply = JSON.parse(data.toString());
      this.settle(`${reply.sessionId || ''}:${reply.id}`);
    } catch {}
  }

  /** Releases every slot still held, as the session ends. */
  async releaseAll() {
    await Promise.allSettled([...this.releases.values()].map((finish) => finish()));
    this.releases.clear();
  }
}
