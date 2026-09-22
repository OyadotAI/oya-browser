/**
 * How a command reaches a browser. Two transports share this interface: the
 * CDP engine is called directly, and an Oya client gets a message over its
 * socket. Each is the `send` of one browser driver (../../driver), which is
 * what callers hold; nothing else picks a transport.
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
  /** A stable reason a caller can branch on, when the failure has one. */
  code?: string;
}

/** Delivers one command and resolves with the browser's answer. */
export interface CommandTransport {
  /** Sends the call; rejects on timeout or a lost connection. */
  send(call: Call): Promise<CommandResult>;
}
