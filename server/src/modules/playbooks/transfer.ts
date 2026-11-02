/**
 * Moving a playbook between environments: an export is the playbook as one JSON
 * document, and an import saves such a document under this key. Secrets travel by
 * name only, as they are stored; their values stay wherever they were entered.
 */
import * as keyConfig from '../config/service.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { MAX_LEN, MAX_STEPS, WORKFLOW_SCHEMA } from './constants.ts';
import { checkName, describe } from './catalog.ts';
import { replayableAction } from './replay.ts';
import { validateWorkflow } from './sanitize.ts';

/** What an export says it is, so an import can tell one from any other JSON. */
export const EXPORT_FORMAT = 'oya-playbook';
/** The export layout's version; an import refuses one it does not know. */
export const EXPORT_VERSION = 1;

/** The fields of a stored playbook that make up the playbook itself, not its history here. */
const CARRIED = ['prompt', 'steps', 'defaults', 'secrets', 'schemaVersion', 'variables'];

/** Only the fields that travel. */
export const carried = (pb) => Object.fromEntries(CARRIED.filter((k) => pb[k] !== undefined).map((k) => [k, pb[k]]));

/**
 * The playbook's fields with every secret's value left out: a classic playbook's
 * defaults for names it lists as secrets, and a workflow's secret variable
 * defaults. Once saved, a secret's value is never sent back, by an export or an
 * editor's read alike.
 */
export function withoutSecretValues(pb) {
  const secrets = new Set((pb.secrets ?? []).map(String));
  const defaults = pb.defaults && Object.fromEntries(Object.entries(pb.defaults).filter(([k]) => !secrets.has(k)));
  const variables =
    pb.variables &&
    Object.fromEntries(
      Object.entries(pb.variables).map(([n, v]: [string, any]) => [n, v?.secret ? { ...v, default: undefined } : v]),
    );
  return { ...pb, ...(defaults ? { defaults } : {}), ...(variables ? { variables } : {}) };
}

/** A saved playbook as a document another environment can import, its secrets' values left out. */
export function exportPlaybook(apiKey, name) {
  const pb = keyConfig.getPlaybook(apiKey, name);
  if (!pb || name.endsWith(':draft')) throw new HttpError(Status.NOT_FOUND, `No playbook named ${name}`);
  const playbook = { name, ...withoutSecretValues(carried(pb)) };
  return { format: EXPORT_FORMAT, version: EXPORT_VERSION, exportedAt: new Date().toISOString(), playbook };
}

/** Refuses a document that is not an export this server can read. */
function checkDocument(doc) {
  if (doc?.format !== EXPORT_FORMAT) throw new HttpError(Status.BAD_REQUEST, `Not an ${EXPORT_FORMAT} export`);
  if (doc.version !== EXPORT_VERSION)
    throw new HttpError(Status.BAD_REQUEST, `Export version ${doc.version} is not supported here`);
}

/** A 400 for a playbook that is not what it should be. */
const refused = (message: string) => new HttpError(Status.BAD_REQUEST, message);

/** A secret's name: what a recording's secrets carry. */
const SECRET_NAME = /^\w{1,64}$/;

/**
 * Refuses the playbook's fields besides its steps when they are not what replay
 * and the playbook list read: a secrets value that is not a list of names broke
 * the list for the key it was saved under.
 */
function checkFields(pb) {
  if (pb.defaults != null && (typeof pb.defaults !== 'object' || Array.isArray(pb.defaults)))
    throw refused('defaults must be an object of values');
  if (pb.secrets != null && (!Array.isArray(pb.secrets) || !pb.secrets.every((k) => SECRET_NAME.test(String(k)))))
    throw refused('secrets must be a list of variable names');
  if (pb.prompt != null && (typeof pb.prompt !== 'string' || pb.prompt.length > MAX_LEN))
    throw refused(`prompt must be text of at most ${MAX_LEN} characters`);
}

/** Refuses steps this server could not replay, naming the first. */
export function checkSteps(pb) {
  const steps = pb?.steps;
  if (!Array.isArray(steps) || !steps.length || steps.length > MAX_STEPS)
    throw refused(`A playbook needs 1 to ${MAX_STEPS} steps`);
  checkFields(pb);
  if (pb.schemaVersion === WORKFLOW_SCHEMA) return void validateWorkflow(pb);
  const bad = steps.findIndex((step) => !step || typeof step !== 'object' || !replayableAction(step.action));
  if (bad >= 0) throw new HttpError(Status.BAD_REQUEST, `Step ${bad + 1} is not a step this server can replay`);
}

/** Refuses to replace a playbook unless asked to. */
function checkFree(apiKey, name, overwrite) {
  if (!overwrite && keyConfig.getPlaybook(apiKey, name))
    throw new HttpError(Status.CONFLICT, `A playbook named ${name} already exists; pass overwrite to replace it`);
}

/** Saves an exported playbook under this key, as `name` or the name it was exported with. */
export async function importPlaybook(apiKey, doc, { name, overwrite = false }: any = {}) {
  checkDocument(doc);
  const target = name ?? doc.playbook?.name;
  checkName(target);
  checkSteps(doc.playbook);
  checkFree(apiKey, target, overwrite);
  const pb = { ...carried(doc.playbook), name: target, importedAt: new Date().toISOString() };
  await keyConfig.savePlaybook(apiKey, target, { defaults: {}, secrets: [], ...pb });
  return describe(keyConfig.getPlaybook(apiKey, target));
}
