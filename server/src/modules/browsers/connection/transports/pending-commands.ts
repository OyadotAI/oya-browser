/**
 * Commands sent over a socket and still waiting for their `cmd_result`, with
 * the timeout that gives up on each.
 */
import { metrics } from '../../../../platform/metrics.ts';
import { reportOutcome, type Call } from '../reporter.ts';
import { MS_PER_SECOND } from '../constants.ts';
import type { CommandResult } from './transport.ts';

/** A command in flight. */
interface Waiter {
  /** The call it answers. */
  call: Call;
  /** When it was sent. */
  startedAt: number;
  /** Gives up on the answer. */
  timer: ReturnType<typeof setTimeout>;
  /** Settles the caller's promise. */
  resolve: (result: CommandResult) => void;
  /** Fails the caller's promise. */
  reject: (error: Error) => void;
}

/** The registry of commands in flight, by command id. */
class PendingCommands {
  /** Command id → waiter. */
  declare private readonly waiters: Map<string, Waiter>;

  /** Starts empty. */
  constructor() {
    this.waiters = new Map();
  }

  /** Waits for the answer to `id`, failing the call after its timeout. */
  wait(id: string, call: Call): Promise<CommandResult> {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => this.expire(id), call.timeout);
      this.waiters.set(id, { call, startedAt: Date.now(), timer, resolve, reject });
      this.report();
    });
  }

  /** Settles the command a `cmd_result` answers; false if nothing (on that browser) waits for it. */
  settle(id: string, browserId: string, result: CommandResult) {
    const waiter = this.take(id, browserId);
    if (!waiter) return false;
    reportOutcome(waiter.call, result.ok ? 'ok' : 'error', Date.now() - waiter.startedAt, result.error);
    waiter.resolve(result);
    return true;
  }

  /** Fails everything waiting on a browser that disconnected or was replaced. */
  failBrowser(browserId: string, outcome: string, message: string) {
    for (const [id, waiter] of this.waiters) {
      if (waiter.call.browserId !== browserId) continue;
      this.take(id, browserId);
      reportOutcome({ ...waiter.call, visible: false }, outcome, Date.now() - waiter.startedAt);
      waiter.reject(new Error(message));
    }
  }

  /** Stops waiting on `id` after a failed send. */
  abandon(id: string) {
    const waiter = this.waiters.get(id);
    if (waiter) this.take(id, waiter.call.browserId);
  }

  /** The answer never came. */
  private expire(id: string) {
    const waiter = this.waiters.get(id);
    if (!waiter) return;
    this.take(id, waiter.call.browserId);
    const { call } = waiter;
    reportOutcome(call, 'timeout', call.timeout, 'timed out');
    waiter.reject(new Error(`Command ${call.action} timed out after ${call.timeout / MS_PER_SECOND}s`));
  }

  /** Removes and returns the waiter for `id` if it belongs to `browserId`. */
  private take(id: string, browserId: string) {
    const waiter = this.waiters.get(id);
    if (!waiter || waiter.call.browserId !== browserId) return null;
    clearTimeout(waiter.timer);
    this.waiters.delete(id);
    this.report();
    return waiter;
  }

  /** Keeps the pending-commands gauge current. */
  private report() {
    metrics.pendingCommands.set({}, this.waiters.size);
  }
}

/** Commands in flight across every socket. */
export const pendingCommands = new PendingCommands();
