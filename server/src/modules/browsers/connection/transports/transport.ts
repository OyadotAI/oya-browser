/**
 * How a command reaches a browser. Two strategies share this interface: an
 * outbound driver is called directly (CDP providers), and an Oya client gets
 * a message over its socket. Callers never branch on which.
 */
import type { Call } from '../reporter.ts';

/** What a browser answers a command with. */
export interface CommandResult {
  /** Whether the command succeeded. */
  ok: boolean;
  /** What it returned. */
  data?: any;
  /** Why it failed. */
  error?: string;
}

/** Delivers one command and resolves with the browser's answer. */
export interface CommandTransport {
  /** Sends the call; rejects on timeout or a lost connection. */
  send(call: Call): Promise<CommandResult>;
}
