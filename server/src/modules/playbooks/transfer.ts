/**
 * Moving a playbook between environments: an export is the playbook as one JSON
 * document, and an import saves such a document under this key. Secrets travel by
 * name only, as they are stored; their values stay wherever they were entered.
 */
import * as keyConfig from '../config/service.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { MAX_STEPS, WORKFLOW_SCHEMA } from './constants.ts';
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
const carried = (pb) => Object.fromEntries(CARRIED.filter((k) => pb[k] !== undefined).map((k) => [k, pb[k]]));

/** A saved playbook as a document another environment can import. */
export function exportPlaybook(apiKey, name) {
  const pb = keyConfig.getPlaybook(apiKey, name);
  if (!pb || name.endsWith(':draft')) throw new HttpError(Status.NOT_FOUND, `No playbook named ${name}`);
  const playbook = { name, ...carried(pb) };
  return { format: EXPORT_FORMAT, version: EXPORT_VERSION, exportedAt: new Date().toISOString(), playbook };
}

/** Refuses a document that is not an export this server can read. */
function checkDocument(doc) {
  if (doc?.format !== EXPORT_FORMAT) throw new HttpError(Status.BAD_REQUEST, `Not an ${EXPORT_FORMAT} export`);
  if (doc.version !== EXPORT_VERSION)
    throw new HttpError(Status.BAD_REQUEST, `Export version ${doc.version} is not supported here`);
}

/** Refuses steps this server could not replay, naming the first. */
function checkSteps(pb) {
  const steps = pb?.steps;
  if (!Array.isArray(steps) || !steps.length || steps.length > MAX_STEPS)
    throw new HttpError(Status.BAD_REQUEST, `A playbook needs 1 to ${MAX_STEPS} steps`);
  if (pb.defaults != null && (typeof pb.defaults !== 'object' || Array.isArray(pb.defaults)))
    throw new HttpError(Status.BAD_REQUEST, 'defaults must be an object of values');
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
