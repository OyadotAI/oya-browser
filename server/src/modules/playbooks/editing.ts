/**
 * Editing a saved playbook in place: reading its whole definition (its steps,
 * defaults, secret names and, for a workflow, its variables) and replacing it.
 * A replacement passes the same checks an import does and is rendered before it
 * is stored, so an edit can never save steps this server could not replay, nor
 * one that would break the playbook list. Nothing here runs code: steps are data
 * a replay matches to the page, the same as a recording's.
 */
import * as keyConfig from '../config/service.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { WORKFLOW_SCHEMA } from './constants.ts';
import { asWorkflow, checkName, describe } from './catalog.ts';
import { carried, checkSteps, withoutSecretValues } from './transfer.ts';

/** The name suffix a healed draft is stored under; a draft is promoted, not edited. */
const DRAFT = ':draft';
/** The fields an edit carries; the rest of a saved playbook (when it was made, healed, labelled) stays. */
const EDITED = ['prompt', 'steps', 'defaults', 'secrets', 'schemaVersion', 'variables'];

/** The saved playbook of that name, or a 404; a draft is not editable. */
function stored(apiKey: string, name: string) {
  const pb = name.endsWith(DRAFT) ? null : keyConfig.getPlaybook(apiKey, name);
  if (!pb) throw new HttpError(Status.NOT_FOUND, `No playbook named ${name}`, { code: 'not_found' });
  return pb;
}

/** The playbook's whole definition, the fields an edit sends back, secrets' values left out. */
export function definitionOf(apiKey: string, name: string) {
  return { name, ...withoutSecretValues(carried(stored(apiKey, name))) };
}

/** A classic playbook's defaults: the edit's, plus the saved value of each secret the definition was sent without. */
function keptDefaults(saved, edited) {
  const secrets = new Set((edited.secrets ?? []).map(String));
  const savedSecrets = Object.entries(saved.defaults ?? {}).filter(([k]) => secrets.has(k));
  return { ...Object.fromEntries(savedSecrets), ...edited.defaults };
}

/** A workflow's variables: each secret left without a default keeps its saved one. */
function keptVariables(saved, edited) {
  const keep = ([n, v]: [string, any]) => [
    n,
    v?.secret && v.default === undefined ? { ...v, default: saved.variables?.[n]?.default } : v,
  ];
  return Object.fromEntries(Object.entries(edited.variables).map(keep));
}

/**
 * Secret values the edit left blank keep the saved ones: the definition was sent
 * without them, so a blank means "unchanged", not "cleared".
 */
function keptSecrets(saved, edited) {
  const defaults = edited.defaults || edited.secrets?.length ? { defaults: keptDefaults(saved, edited) } : {};
  return { ...edited, ...defaults, ...(edited.variables ? { variables: keptVariables(saved, edited) } : {}) };
}

/** The definition a body carries, refused when it is not an object. */
function definitionIn(body) {
  if (!body || typeof body !== 'object' || Array.isArray(body))
    throw new HttpError(Status.BAD_REQUEST, 'The body must be the playbook definition', { code: 'invalid_request' });
  return carried(body);
}

/** `saved` without the fields an edit carries, so a field the edit removed is not kept. */
const history = (saved) => Object.fromEntries(Object.entries(saved).filter(([k]) => !EDITED.includes(k)));

/**
 * The playbook an edit stores: its history, the edited fields with blank secrets
 * kept, and for a workflow its defaults worked out from its variables again, the
 * way a recorded workflow's are, since replay reads a workflow's variables.
 */
function nextPlaybook(saved, edited, name: string) {
  const next = { ...history(saved), ...keptSecrets(saved, edited), name, updatedAt: new Date().toISOString() };
  if (next.schemaVersion === WORKFLOW_SCHEMA) asWorkflow(next, next);
  return next;
}

/** Replaces a saved playbook's definition; its name, history and labels stay. */
export async function replacePlaybook(apiKey: string, name: string, body) {
  checkName(name);
  const edited = definitionIn(body);
  checkSteps(edited);
  const next = nextPlaybook(stored(apiKey, name), edited, name);
  describe(next);
  await keyConfig.savePlaybook(apiKey, name, next);
  return describe(keyConfig.getPlaybook(apiKey, name));
}

/** An unsaved definition a run tests, checked like a save; it is never stored, even when healed. */
export function draftPlaybook(body) {
  const edited = definitionIn(body);
  checkSteps(edited);
  return { defaults: {}, secrets: [], ...edited, name: 'draft', transient: true };
}
