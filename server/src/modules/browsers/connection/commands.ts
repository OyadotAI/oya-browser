/**
 * Facade for sending commands to browsers: picks the transport strategy for
 * the browser, runs it inside the control plane's command slot, and turns a
 * lost answer into a clear outcome.
 */
import { control } from '../../control/service.ts';
import { registry } from '../registry.ts';
import { HttpError } from '../../../platform/errors.ts';
import { CdpConnectionError } from '../../../drivers/cdp.ts';
import { Status } from '../../../platform/http-status.ts';
import { canonical, supportedOn, VOCABULARY } from '../../../drivers/vocabulary.ts';
import { describeCall } from './reporter.ts';
import { noteDialog } from './dialog-notes.ts';
import { COMMAND_TIMEOUT_MS, NAVIGATE_TIMEOUT_MS } from './constants.ts';
import { pendingCommands } from './transports/pending-commands.ts';
import type { CommandResult } from './transports/transport.ts';

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

/** A lost answer becomes 504 `command_outcome_unknown`; other errors pass through. A lost CDP connection is one whatever its words. */
function asOutcomeUnknown(e: Error) {
  if (e instanceof HttpError) return e;
  if (!(e instanceof CdpConnectionError) && !OUTCOME_UNKNOWN.test(e.message)) return e;
  return new HttpError(Status.GATEWAY_TIMEOUT, e.message, { code: 'command_outcome_unknown' });
}

/** Runs the command through the browser's transport, once the vocabulary says this browser can do it. */
function dispatch(browserId: string, action: string, params: object, timeoutMs?: number): Promise<CommandResult> {
  const browser = registry.get(browserId);
  if (!browser)
    return Promise.reject(new HttpError(Status.NOT_FOUND, `Browser ${browserId} not connected`, { code: 'not_found' }));
  const refused = refusalFor(browser, action);
  if (refused) return Promise.reject(refused);
  const call = describeCall(browserId, action, params, timeoutMs || defaultTimeout(action));
  if (call.visible) registry.commandStarted(browserId);
  return browser.driver.send(call);
}

/**
 * Why this browser cannot be asked to do `action`, or null when it can. Checked
 * here, before the browser is asked, so the words have one source and a
 * refusal touches none of the browser's counters. A name the browser itself
 * announced always passes: a newer app may do more than this list knows.
 */
function refusalFor(browser, action: string): HttpError | null {
  const announced = browser.driver.actions();
  const known = canonical(action);
  if (announced.includes(action) || (known && announced.includes(known))) return null;
  if (!known) return unknownAction(action);
  return VOCABULARY[known][browser.clientType] ? null : unsupportedAction(browser, action, known);
}

/** An action no browser does. */
const unknownAction = (action: string) =>
  new HttpError(
    Status.BAD_REQUEST,
    `Unknown action ${JSON.stringify(action)}. The browser detail lists the actions this browser supports, in "actions".`,
    { code: 'action_unknown', action },
  );

/** "an oya", "a cdp": the article before a kind's name. */
const withArticle = (kind: string) => `${/^[aeiou]/.test(kind) ? 'an' : 'a'} ${kind}`;

/** An action this kind of browser does not do, with the kinds that do it. */
function unsupportedAction(browser, action: string, known: string) {
  const kinds = supportedOn(known);
  if (!kinds.length) return unknownAction(action);
  const where = `${browser.clientType} browsers (this one is ${browser.provider || browser.clientType})`;
  const next = `Use ${withArticle(kinds[0])} browser for this step, or pick another action.`;
  const message = `The action ${JSON.stringify(action)} is not supported on ${where}. Supported on: ${kinds.join(', ')}. ${next}`;
  return new HttpError(Status.UNPROCESSABLE, message, unsupportedFields(browser, action, kinds));
}

/** The fields a caller branches on when an action is not supported here: which browser, and where it is supported. */
const unsupportedFields = ({ clientType, provider }, action: string, supportedOn: string[]) => ({
  code: 'action_unsupported',
  action,
  clientType,
  provider,
  supportedOn,
});

/** How long a command may take when the caller does not say. */
const defaultTimeout = (action: string) => (action === 'navigate' ? NAVIGATE_TIMEOUT_MS : COMMAND_TIMEOUT_MS);

/** Settles the command a `cmd_result` answers, if the socket it came on is the browser's current one. */
export function settleResult(browserId: string, msg, fromCurrentSocket: boolean) {
  const code = msg.ok === false ? msg.code || 'command_failed' : undefined;
  const result = { ok: msg.ok, data: msg.data, error: msg.error, code };
  const settled = fromCurrentSocket && pendingCommands.settle(msg.id, browserId, result);
  const note = settled ? `ok=${msg.ok}` : '(no pending command, stale or timed out)';
  console.log(`[ws] ← cmd_result from ${browserId}: id=${msg.id} ${note}`);
  if (settled) noteDialog(browserId, msg);
}

/** Fails every command still waiting on a browser that disconnected or was replaced. */
export function failPending(browserId: string, outcome: 'disconnected' | 'reconnected', message: string) {
  pendingCommands.failBrowser(browserId, outcome, message);
}
