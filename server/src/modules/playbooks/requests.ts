/**
 * Checking the bodies of the playbook and run routes before any work starts.
 * Each check answers with the refusal the route sends, or what the route needs.
 */
import * as keyConfig from '../config/service.ts';
import { Status } from '../../platform/http-status.ts';
import { validData } from '../../app/http.ts';
import { MAX_LEN, WORKFLOW_SCHEMA } from './constants.ts';
import { missingVariables, sanitizeSteps, validateWorkflow } from './service.ts';

/** The playbook a request names, or the status and message it is refused with. */
interface Checked {
  /** The playbook, when the request names one and may run. */
  pb?: any;
  /** The refusal's HTTP status. */
  status?: number;
  /** The refusal's message. */
  error?: string;
}

/** A refusal to send. */
const refuse = (status: number, error: string): Checked => ({ status, error });

/** Why a save request is refused, or null. */
export function saveRefusal({ steps, secrets, schemaVersion }): string | null {
  if (schemaVersion !== undefined && schemaVersion !== WORKFLOW_SCHEMA) return 'Unsupported workflow version';
  const badSecrets = !Array.isArray(secrets) || secrets.some((k) => !/^\w{1,64}$/.test(String(k)));
  if (steps !== undefined && secrets !== undefined && badSecrets) return 'secrets must be an array of variable names';
  return null;
}

/** The run a recording in the body becomes; undefined saves the browser's last ask() instead. */
export function recordedRun(body) {
  if (body.steps === undefined) return undefined;
  return {
    prompt: String(body.prompt || body.name || '').slice(0, MAX_LEN),
    ...recordedSteps(body),
    secrets: (body.secrets || []).map(String),
  };
}

/** The recorded steps, checked: a workflow draft with its variables, or sanitized classic steps. */
function recordedSteps({ steps, secrets, schemaVersion, variables }) {
  if (schemaVersion !== WORKFLOW_SCHEMA) return { steps: sanitizeSteps(steps) };
  const draft = validateWorkflow({ schemaVersion, steps, variables, secrets });
  return { steps: draft.steps, schemaVersion, variables: draft.variables };
}

/** The playbook a play request names, when its variables are valid and complete. */
export function playable(key, name, variables): Checked {
  if (!validData(variables))
    return refuse(Status.BAD_REQUEST, 'variables must map names to strings, numbers or a file() value');
  const pb = keyConfig.getPlaybook(key, name);
  if (!pb) return refuse(Status.NOT_FOUND, `No playbook named ${name}`);
  const missing = missingVariables(pb, variables);
  if (missing.length) return refuse(Status.BAD_REQUEST, `Missing variables: ${missing.join(', ')}`);
  return { pb };
}

/** The playbook a run request names (null for a prompt), when its data is valid and complete. */
export function runnable(key, { prompt, playbook, data, secrets }): Checked {
  if (!validData(data) || !validData(secrets, { files: false }))
    return refuse(
      Status.BAD_REQUEST,
      'data must map names to strings, numbers or a file() value; secrets takes strings and numbers only',
    );
  if ((typeof prompt === 'string') === (typeof playbook === 'string'))
    return refuse(Status.BAD_REQUEST, 'Pass exactly one of prompt or playbook');
  return runnablePlaybook(key, playbook, { ...data, ...secrets });
}

/** The named playbook, when it exists and `values` covers what it needs. */
function runnablePlaybook(key, playbook, values): Checked {
  const pb = playbook ? keyConfig.getPlaybook(key, playbook) : null;
  if (playbook && !pb) return refuse(Status.NOT_FOUND, `No playbook named ${playbook}`);
  const missing = pb ? missingVariables(pb, values) : [];
  if (missing.length) return refuse(Status.BAD_REQUEST, `Missing data: ${missing.join(', ')}`);
  return { pb };
}
