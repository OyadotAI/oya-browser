/**
 * A key's saved playbooks: saving a run by name, renaming, promoting a healed
 * draft, listing and deleting. A healed draft is stored as `<name>:draft`.
 */
import workflow from '../../../../browser/scripts/workflow.cjs';

import { lastRun, hasReplayableSteps } from '../agent/chat.ts';
import * as keyConfig from '../config/service.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { WORKFLOW_SCHEMA } from './constants.ts';
import { namesOf, templateValues } from './variables.ts';
import { renderPlaywright } from './playwright.ts';

/** A valid playbook name. */
const NAME = /^[\w-]{1,64}$/;

/** What callers see of a playbook: its inputs and its Playwright export. */
const describe = (pb) => ({
  name: pb.name,
  variables: namesOf(pb),
  defaults: pb.defaults || {},
  steps: pb.steps.length,
  code: renderPlaywright(pb),
});

/** Refuses a name that is not 1-64 letters, digits, _ or -. */
function checkName(name) {
  if (typeof name !== 'string' || !NAME.test(name))
    throw new HttpError(Status.BAD_REQUEST, 'Playbook name must be 1-64 letters, digits, _ or -');
}

/** Refuses a run with nothing to replay, saying whether there was a run at all. */
function checkReplayable(run) {
  if (hasReplayableSteps(run)) return;
  const why = run
    ? 'Nothing to replay: this run only visited pages. Playbooks replay clicks, typing and other actions.'
    : 'Nothing to save: run ask() on this browser first.';
  throw new HttpError(Status.CONFLICT, why);
}

/** `run` defaults to the browser's last ask(); a recording passes its own steps in. */
export async function create(apiKey, browserId, name, run = lastRun(browserId)) {
  checkName(name);
  checkReplayable(run);
  const pb = fromRun(name, run);
  await keyConfig.savePlaybook(apiKey, name, pb);
  return describe(pb);
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
function asWorkflow(pb, run) {
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
