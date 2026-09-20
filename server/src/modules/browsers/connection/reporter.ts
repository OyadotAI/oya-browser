/**
 * The one place a command's outcome is recorded: metrics for both client
 * types, and the browser's activity log for commands a person would recognise.
 */
import { registry, summarise } from '../registry.ts';
import { metrics } from '../../../platform/metrics.ts';

/** A command on its way to a browser. */
export interface Call {
  /** Browser it goes to. */
  browserId: string;
  /** Command name. */
  action: string;
  /** Command arguments. */
  params: object;
  /** Milliseconds before giving up. */
  timeout: number;
  /** Whether it belongs in the activity log. */
  visible: boolean;
  /** One-line description for the activity log. */
  summary: string;
}

/**
 * Server-internal actions are not what the browser is "doing". `record` is a poll,
 * several a second while someone records: logged, it would evict every real action.
 */
const HIDDEN_ACTIONS = new Set(['evaluate_raw', 'record']);

/** Describes a command for sending and reporting. */
export function describeCall(browserId: string, action: string, params: object, timeout: number): Call {
  const visible = !HIDDEN_ACTIONS.has(action);
  return { browserId, action, params, timeout, visible, summary: summarise(action, params) };
}

/** Counts one command outcome; both transports report through here so the numbers compare. */
export function recordCommand(action: string, outcome: string, ms: number) {
  metrics.commands.inc({ action, outcome });
  metrics.commandDuration.observe({ action }, ms);
}

/** Records a finished command: metrics always, the activity log when visible. */
export function reportOutcome(call: Call, outcome: string, ms: number, error?: string) {
  recordCommand(call.action, outcome, ms);
  if (!call.visible) return;
  const ok = outcome === 'ok';
  registry.recordActivity(call.browserId, { action: call.action, summary: call.summary, ok, ms, error });
}
