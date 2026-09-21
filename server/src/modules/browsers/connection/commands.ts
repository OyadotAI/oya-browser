/**
 * Facade for sending commands to browsers: picks the transport strategy for
 * the browser, runs it inside the control plane's command slot, and turns a
 * lost answer into a clear outcome.
 */
import { control } from '../../control/service.ts';
import { registry } from '../registry.ts';
import { HttpError } from '../../../platform/errors.ts';
import { Status } from '../../../platform/http-status.ts';
import { describeCall } from './reporter.ts';
import { noteDialog } from './dialog-notes.ts';
import { COMMAND_TIMEOUT_MS, NAVIGATE_TIMEOUT_MS } from './constants.ts';
import { DriverTransport } from './transports/driver-transport.ts';
import { SocketTransport } from './transports/socket-transport.ts';
import { pendingCommands } from './transports/pending-commands.ts';
import type { CommandResult, CommandTransport } from './transports/transport.ts';

export type { CommandResult } from './transports/transport.ts';
export { takeDialogNote } from './dialog-notes.ts';

/** Errors after which nobody knows whether the command ran. */
const OUTCOME_UNKNOWN = /timed out|timeout|disconnect|reconnect/i;

/**
 * Sends a command and waits for its result, inside the control plane's command
 * slot. A command lost to a timeout or disconnect answers 504
 * `command_outcome_unknown`: it may or may not have run.
 */
export async function sendCommand(browserId: string, action: string, params = {}, timeoutMs?: number, holder = null) {
  const finish = await control().beginCommand(browserId, holder);
  try {
    return await dispatch(browserId, action, params, timeoutMs);
  } catch (e) {
    throw asOutcomeUnknown(e);
  } finally {
    await finish();
  }
}

/** A lost answer becomes 504 `command_outcome_unknown`; other errors pass through. */
function asOutcomeUnknown(e: Error) {
  if (!OUTCOME_UNKNOWN.test(e.message)) return e;
  return new HttpError(Status.GATEWAY_TIMEOUT, e.message, { code: 'command_outcome_unknown' });
}

/** Runs the command through the browser's transport. */
function dispatch(browserId: string, action: string, params: object, timeoutMs?: number): Promise<CommandResult> {
  const browser = registry.get(browserId);
  if (!browser) return Promise.reject(new Error(`Browser ${browserId} not connected`));
  const call = describeCall(browserId, action, params, timeoutMs || defaultTimeout(action));
  if (call.visible) registry.commandStarted(browserId);
  return transportFor(browser).send(call);
}

/** The strategy for this browser: its driver if we drive it, else its socket. */
function transportFor(browser): CommandTransport {
  return browser.driver ? new DriverTransport(browser.driver) : new SocketTransport(browser.ws);
}

/** How long a command may take when the caller does not say. */
const defaultTimeout = (action: string) => (action === 'navigate' ? NAVIGATE_TIMEOUT_MS : COMMAND_TIMEOUT_MS);

/** Settles the command a `cmd_result` answers, if the socket it came on is the browser's current one. */
export function settleResult(browserId: string, msg, fromCurrentSocket: boolean) {
  const result = { ok: msg.ok, data: msg.data, error: msg.error };
  const settled = fromCurrentSocket && pendingCommands.settle(msg.id, browserId, result);
  const note = settled ? `ok=${msg.ok}` : '(no pending command, stale or timed out)';
  console.log(`[ws] ← cmd_result from ${browserId}: id=${msg.id} ${note}`);
  if (settled) noteDialog(browserId, msg);
}

/** Fails every command still waiting on a browser that disconnected or was replaced. */
export function failPending(browserId: string, outcome: 'disconnected' | 'reconnected', message: string) {
  pendingCommands.failBrowser(browserId, outcome, message);
}
