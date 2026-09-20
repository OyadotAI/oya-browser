/**
 * Commands for Oya clients: a `cmd` over the browser's own socket, answered
 * later by a `cmd_result` that pendingCommands settles.
 */
import { randomUUID } from 'crypto';
import type { Call } from '../reporter.ts';
import type { CommandResult, CommandTransport } from './transport.ts';
import { pendingCommands } from './pending-commands.ts';

/** Sends commands over one browser's socket. */
export class SocketTransport implements CommandTransport {
  /** The browser's control socket. */
  declare private readonly ws: { send(data: string): void };

  /** Wraps a connected browser's socket. */
  constructor(ws) {
    this.ws = ws;
  }

  /** Sends the call and waits for its result (or its timeout); a failed write fails it at once. */
  send(call: Call): Promise<CommandResult> {
    const id = randomUUID();
    const answer = pendingCommands.wait(id, call);
    console.log(`[ws] → cmd to ${call.browserId}: id=${id} action=${call.action}`);
    const failed = this.post(id, call);
    if (!failed) return answer;
    pendingCommands.abandon(id);
    return Promise.reject(failed);
  }

  /** Writes the `cmd` message; returns the write error, if any. */
  private post(id: string, call: Call): Error | null {
    try {
      this.ws.send(JSON.stringify({ type: 'cmd', id, action: call.action, params: call.params }));
      return null;
    } catch (err) {
      return err;
    }
  }
}
