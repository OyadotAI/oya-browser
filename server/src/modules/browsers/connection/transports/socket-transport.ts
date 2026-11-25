/**
 * Commands for Oya clients: a `cmd` over the browser's own socket, answered
 * later by a `cmd_result` that pendingCommands settles.
 */
import { randomUUID } from 'crypto';
import { HttpError } from '../../../../platform/errors.ts';
import { Status } from '../../../../platform/http-status.ts';
import type { Call } from '../reporter.ts';
import type { CommandResult, CommandTransport } from './transport.ts';
import { pendingCommands } from './pending-commands.ts';

/** The message types a call may go out as: a command, a profile capture, or a model call to relay. */
type Wire = 'cmd' | 'profile_capture' | 'llm_request';

/** Sends commands over one browser's socket. */
export class SocketTransport implements CommandTransport {
  /** The browser's control socket. */
  declare private readonly ws: { send(data: string): void };

  /** Wraps a connected browser's socket. */
  constructor(ws) {
    this.ws = ws;
  }

  /** Sends the call and waits for its result (or its timeout); a failed write fails it at once. */
  send(call: Call, type: Wire = 'cmd'): Promise<CommandResult> {
    const id = randomUUID();
    const answer = pendingCommands.wait(id, call);
    console.log(`[ws] → cmd to ${call.browserId}: id=${id} action=${call.action}`);
    const failed = this.post(id, call, type);
    if (!failed) return answer;
    pendingCommands.abandon(id, failed.message);
    return Promise.reject(disconnectedError());
  }

  /** Writes the `cmd` message; returns the write error, if any. */
  private post(id: string, call: Call, type: Wire): Error | null {
    try {
      this.ws.send(JSON.stringify({ type, id, action: call.action, params: call.params, lane: call.lane }));
      return null;
    } catch (err) {
      return err;
    }
  }
}

/** Clean 503 error raised when the browser socket fails before a command leaves the server. */
const disconnectedError = () =>
  new HttpError(Status.UNAVAILABLE, 'Browser disconnected before the command could be sent', {
    code: 'browser_disconnected',
  });
