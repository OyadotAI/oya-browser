/**
 * Playbooks: an ask() run frozen into steps that replay without the LLM.
 *
 * Steps carry the analyzer's stable element metadata (testId, DOM id, aria-label,
 * text, name), never the numeric id, which dies with each analysis. Every value that
 * was typed or picked is a `{{name}}` placeholder filled at replay, with the
 * recorded value kept as its default, so a playbook is a form, not a transcript, and
 * a run that passes nothing still does what was demonstrated. Passwords are the one
 * value with no default: they never leave the page. Replay re-analyzes and matches,
 * so it works on every provider. The Playwright code is an export to read or run
 * yourself; nothing here ever evaluates it.
 *
 * This is the module's entry point; the work lives in the files it re-exports.
 */
export { matchElement } from './match.ts';
export { sanitizeSteps, validateWorkflow } from './sanitize.ts';
export { variablesOf, missingVariables, templateValues } from './variables.ts';
export { renderPlaywright } from './playwright.ts';
export { create, promote, rename, list, remove } from './catalog.ts';
export { exportPlaybook, importPlaybook, EXPORT_FORMAT, EXPORT_VERSION } from './transfer.ts';
export { answerField } from './answer.ts';
export { runsOf, lastRuns, type RunRecord } from './history.ts';
import { play as replay } from './replay.ts';
import { record, type RunRecord } from './history.ts';
import { track } from '../telemetry/index.ts';
import { fingerprint } from '../../platform/audit.ts';
import { getPlaybook } from '../config/service.ts';

/** How a replay ended: the agent finished it (healed), a person did (handed over), the steps ran as saved (ok), or it threw. */
export function outcomeOf(result, failed: boolean) {
  if (failed) return 'failed';
  if (!result?.fellBack) return 'ok';
  return result.healed ? 'healed' : 'handed_over';
}

/** Counts how a replay went, whoever asked (a route or a job). */
function counted(apiKey, steps: number, result?, failed = false) {
  track.playbookReplayed(apiKey, { outcome: outcomeOf(result, failed), steps, healed: !!result?.healed });
}

/** Where a replay ran and when it began, for its history. */
interface RunStart {
  /** The browser it ran on. */
  browserId: string;
  /** When it began, as an ISO time. */
  startedAt: string;
}

/** What a finished replay's history entry is made from. */
interface Finished extends RunStart {
  /** Why it failed, if it did. */
  err?: unknown;
  /** The values it ran with, so a secret quoted in its error is kept out of the history. */
  vars: Record<string, unknown>;
}

/**
 * Keeps a finished replay in its playbook's history. Best effort: a failed write is
 * logged and never fails the run. Left out: the editor's tester runs (unsaved edits,
 * no playbook's runs), and a run whose playbook was deleted or renamed while it ran,
 * which would otherwise hand its runs to the next playbook given that name.
 */
function remember(apiKey, pb, done: Finished) {
  if (!pb?.name || pb.transient || !getPlaybook(apiKey, pb.name)) return;
  void record(fingerprint(apiKey), pb.name, finished(pb, done)).catch((e) =>
    console.warn(`[playbooks] run history not saved: ${e.message}`),
  );
}

/** The run as its history keeps it: how it ended, and why when it failed. */
function finished(pb, { err, vars, browserId, startedAt }: Finished): RunRecord {
  const run = { browserId, startedAt, endedAt: new Date().toISOString() };
  if (!err) return { status: 'succeeded', ...run };
  return { status: 'failed', ...run, error: withoutSecrets(String((err as Error).message ?? err), pb, vars) };
}

/** The text with every secret value the run was given blanked, since an error can quote what was typed. */
function withoutSecrets(text: string, pb, vars: Record<string, unknown>) {
  const secrets = (pb.secrets ?? []).map((name) => vars[name]).filter((v) => typeof v === 'string' && v);
  return secrets.reduce((out: string, v) => out.split(v as string).join('[secret]'), text);
}

/** Replays a playbook, counts the outcome and keeps it in the history, success or failure. */
export function play(apiKey, browserId, pb, vars = {}, options = {}) {
  const steps = pb?.steps?.length ?? 0;
  const start = { browserId, startedAt: new Date().toISOString(), vars };
  return replay(apiKey, browserId, pb, vars, options).then(
    (result) => (counted(apiKey, steps, result), remember(apiKey, pb, start), result),
    (err) => (counted(apiKey, steps, undefined, true), remember(apiKey, pb, { ...start, err }), Promise.reject(err)),
  );
}
