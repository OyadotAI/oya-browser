/**
 * A key's saved playbooks: saving a run by name, renaming, promoting a healed
 * draft, listing and deleting. A healed draft is stored as `<name>:draft`.
 */
import * as workflow from '../../../../browser/src/workflow/index.ts';

import { lastRun } from '../agent/chat.ts';
import * as keyConfig from '../config/service.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { MAX_STEPS, WORKFLOW_SCHEMA } from './constants.ts';
import { replayableAction } from './replay.ts';
import { namesOf, templateValues } from './variables.ts';
import { renderPlaywright } from './playwright.ts';

/** A valid playbook name. */
const NAME = /^[\w-]{1,64}$/;

/**
 * The playbook's defaults without its secrets' values: a list or a save answer
 * reaches the console and SDK callers, and a secret's value is never sent back
 * once saved. A run that leaves a secret out still uses the saved one.
 */
const shownDefaults = (pb) => {
  const secrets = new Set(secretNames(pb));
  return Object.fromEntries(Object.entries(pb.defaults || {}).filter(([k]) => !secrets.has(k)));
};

/**
 * The names (never the values) of the playbook's secrets, recorded or declared
 * on a workflow's variables, so a form that sets its values (a routine's) can say
 * the saved value is used instead of asking for it again.
 */
const secretNames = (pb): string[] => {
  const declared = Object.entries(pb.variables || {}).filter(([, v]: [string, any]) => v?.secret);
  return [...new Set([...(pb.secrets ?? []).map(String), ...declared.map(([k]) => k)])];
};

/** What callers see of a playbook: its inputs and its Playwright export. */
export const describe = (pb) => ({
  name: pb.name,
  variables: namesOf(pb),
  defaults: shownDefaults(pb),
  secrets: secretNames(pb),
  steps: pb.steps.length,
  // The free-text fields the model writes fresh on each run, by variable and question.
  answers: pb.steps.filter((step) => step.answer).map(({ answer }) => ({ key: answer.key, question: answer.question })),
  code: renderPlaywright(pb),
});

/** Refuses a name that is not 1-64 letters, digits, _ or -. */
export function checkName(name) {
  if (typeof name !== 'string' || !NAME.test(name))
    throw new HttpError(Status.BAD_REQUEST, 'Playbook name must be 1-64 letters, digits, _ or -');
}

/** Refuses a run with nothing to replay, saying whether there was a run at all. */
function checkReplayable(run, steps) {
  if (steps.some((s) => s.action !== 'navigate')) return;
  const why = run
    ? 'Nothing to replay: this run only visited pages. Playbooks replay clicks, typing and other actions.'
    : 'Nothing to save: run ask() on this browser first.';
  throw new HttpError(Status.CONFLICT, why);
}

/** `run` defaults to the browser's last ask(); a recording passes its own steps in. */
export async function create(apiKey, browserId, name, run = lastRun(browserId)) {
  checkName(name);
  const steps = replayableSteps(run);
  checkReplayable(run, steps);
  const pb = fromRun(name, { ...run, steps });
  await keyConfig.savePlaybook(apiKey, name, pb);
  return describe(pb);
}

/**
 * The run's steps as recorded, less any action replay has no handler for.
 *
 * Not sanitizeSteps(): that is for a recording built in a web page (requests.ts runs
 * it on those before they get here). The agent's run comes from this server's own
 * recorder, and passing it through the page sanitizer kept 7 of the actions the
 * recorder records, so typing into a focused field (keyboard_type), a key chord or a
 * tab switch vanished on save and the replay "succeeded" without them. Fields are
 * kept whole too: cutting typed text short would corrupt a playbook just as quietly.
 */
function replayableSteps(run) {
  if (!run) return [];
  if (run.schemaVersion === WORKFLOW_SCHEMA) return structuredClone(run.steps || []);
  const steps = Array.isArray(run.steps) ? run.steps : [];
  const kept = withoutRunUp(steps.filter((s) => replayableAction(s?.action)));
  // Refused, not cut: a playbook missing its tail would replay as if it were whole.
  if (kept.length > MAX_STEPS) throw new HttpError(Status.BAD_REQUEST, `A playbook is limited to ${MAX_STEPS} steps`);
  return structuredClone(kept);
}

/** Steps that change something on the page; the rest only move around it. */
const ACTS = new Set(['type', 'keyboard_type', 'select_option', 'upload_file', 'click', 'double_click', 'edit_text']);
/** Steps that change which page the run is on, and the field that says which page it is now. */
const FRESH: Record<string, string> = {
  navigate: 'url',
  reload: '',
  open_tab: 'url',
  switch_tab: 'tabUrl',
  // Closing a tab leaves the run on a page the recording does not name.
  close_tab: 'none',
};

/**
 * The run from its last change of page (a navigation, reload, new tab or tab switch)
 * before its first real action, opened with a navigation to the page it was then on. An agent often looks around first,
 * scrolling, opening and closing tabs, pressing Tab, reloading, and a playbook that kept
 * that run-up replayed every bit of it and read as nonsense in its Playwright file.
 * Nothing after the first real action is touched: a tab switch mid-task still replays.
 */
function withoutRunUp(steps) {
  const { start, url } = lastFreshStart(
    steps,
    steps.findIndex((s) => ACTS.has(s.action)),
  );
  if (start < 1 || !url) return steps;
  return [{ action: 'navigate', url, start: true }, ...steps.slice(start + 1)];
}

/** Where a run last started afresh. */
interface FreshStart {
  /** The step's index, or -1 for none. */
  start: number;
  /** The page it was on. */
  url?: string;
}

/** The last change of page before step `first`, and the page it left the run on (a reload stays on the same one). */
function lastFreshStart(steps, first: number): FreshStart {
  let start = -1;
  let url;
  for (let i = 0; i < first; i++) {
    if (!Object.hasOwn(FRESH, steps[i].action)) continue;
    start = i;
    // A reload stays put; a step that does not say where it left the run makes the page unknown.
    url = FRESH[steps[i].action] === '' ? url : steps[i][FRESH[steps[i].action]];
  }
  return { start, url };
}

/** The playbook a run becomes. */
function fromRun(name, run) {
  // Which keys were secrets, so a healing replay keeps them hidden from the model.
  const secrets = run.secrets || [];
  const createdAt = new Date().toISOString();
  const pb: any = { name, prompt: run.prompt, steps: structuredClone(run.steps), defaults: {}, secrets, createdAt };
  if (run.schemaVersion === WORKFLOW_SCHEMA) asWorkflow(pb, run);
  else templateValues(pb);
  return pb;
}

/** A workflow run keeps its declared variables; their non-secret defaults become the playbook's. */
export function asWorkflow(pb, run) {
  pb.schemaVersion = WORKFLOW_SCHEMA;
  pb.variables = run.variables || {};
  pb.defaults = Object.fromEntries(
    Object.entries(pb.variables)
      .filter(([, v]: [string, any]) => !v.secret && v.default !== undefined)
      .map(([k, v]: [string, any]) => [k, v.default]),
  );
  workflow.generate(pb);
}

/** Make a healed draft the playbook. */
export async function promote(apiKey, name) {
  const draft = keyConfig.getPlaybook(apiKey, `${name}:draft`);
  if (!draft) throw new HttpError(Status.NOT_FOUND, `No healed draft for ${name}`);
  const { healedAt, healedFrom, ...pb } = draft;
  await keyConfig.savePlaybook(apiKey, name, { ...pb, name, promotedAt: new Date().toISOString() });
  await keyConfig.deletePlaybook(apiKey, `${name}:draft`);
  return describe({ ...pb, name });
}

/** Rename a playbook; its healed draft moves with it. */
export async function rename(apiKey, name, newName) {
  checkName(newName);
  const pb = renameable(apiKey, name);
  if (newName === name) return describe(pb);
  checkFree(apiKey, newName);
  await keyConfig.savePlaybook(apiKey, newName, { ...pb, name: newName });
  await moveDraft(apiKey, name, newName);
  await keyConfig.deletePlaybook(apiKey, name);
  return describe({ ...pb, name: newName });
}

/** The playbook to rename; a draft is not one. */
function renameable(apiKey, name) {
  const pb = keyConfig.getPlaybook(apiKey, name);
  if (!pb || name.endsWith(':draft')) throw new HttpError(Status.NOT_FOUND, `No playbook named ${name}`);
  return pb;
}

/** Refuses a name another playbook already has. */
function checkFree(apiKey, newName) {
  if (keyConfig.getPlaybook(apiKey, newName))
    throw new HttpError(Status.CONFLICT, `A playbook named ${newName} already exists`);
}

/** Moves a playbook's healed draft, if it has one, to the new name. */
async function moveDraft(apiKey, name, newName) {
  const draft = keyConfig.getPlaybook(apiKey, `${name}:draft`);
  if (!draft) return;
  await keyConfig.savePlaybook(apiKey, `${newName}:draft`, { ...draft, name: `${newName}:draft` });
  await keyConfig.deletePlaybook(apiKey, `${name}:draft`);
}

/** Every playbook, newest first, each with the healed draft waiting for review. */
export function list(apiKey) {
  const all = keyConfig.listPlaybooks(apiKey);
  const drafts = new Map(
    all.filter((p) => p.name.endsWith(':draft')).map((p) => [p.name.slice(0, -':draft'.length), p]),
  );
  return all
    .filter((p) => !p.name.endsWith(':draft'))
    .map((pb) => listed(pb, drafts.get(pb.name)))
    .sort((a, b) => String(b.createdAt).localeCompare(String(a.createdAt)));
}

/** A playbook as listed, with its draft. */
function listed(pb, draft) {
  return {
    ...describe(pb),
    createdAt: pb.createdAt || null,
    promotedAt: pb.promotedAt || null,
    draft: draft ? { ...describe(draft), healedAt: draft.healedAt, healedFrom: draft.healedFrom } : null,
  };
}

/** Delete a playbook and its draft, or just a draft when named `<name>:draft`. */
export async function remove(apiKey, name) {
  if (!keyConfig.getPlaybook(apiKey, name)) throw new HttpError(Status.NOT_FOUND, `No playbook named ${name}`);
  await keyConfig.deletePlaybook(apiKey, name);
  if (!name.endsWith(':draft')) await keyConfig.deletePlaybook(apiKey, `${name}:draft`);
}
