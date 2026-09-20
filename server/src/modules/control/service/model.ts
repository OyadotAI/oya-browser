/**
 * The control plane's vocabulary: this replica's id, session states, project
 * ids derived from keys, and the coded error every operation throws.
 */
import { createHash, randomUUID } from 'node:crypto';
import { HttpError } from '../../../platform/errors.ts';
import { Status } from '../../../platform/http-status.ts';
import { PROJECT_ID_CHARS } from './constants.ts';

/** This replica's id; leases and session ownership are recorded against it. */
export const instanceId = process.env.OYA_INSTANCE_ID || randomUUID();
/** States a session never leaves. */
export const terminal = new Set(['stopped', 'failed']);
/** Every non-terminal state: the session may still own a resource, a slot, or a queue position. */
export const live = [
  'queued',
  'provisioning',
  'ready',
  'disconnected',
  'stopping',
  'cleanup_pending',
  'unknown_outcome',
];
/** Providers with no resource of ours to delete: a failed or lost session is simply over. */
export const attachOnly = new Set(['cdp', 'oya-desktop', 'gateway']);
/** Capacity is held while a resource may exist. Disconnected sessions have no deletion descriptor and re-check capacity on reconnect. */
export const holdsSlot = (x) => !terminal.has(x.state) && x.state !== 'queued' && x.state !== 'disconnected';
/** SHA-256 hex. */
export const hash = (value) => createHash('sha256').update(value).digest('hex');
/** The project an API key opens, derived from the key so no lookup is needed. */
export const projectId = (key) => `prj_${hash(key).slice(0, PROJECT_ID_CHARS)}`;
/** An Error carrying an API error code and HTTP status. */
export const fault = (code, message, status: number = Status.CONFLICT) => new HttpError(status, message, { code });
/** Now, in milliseconds. */
export const stamp = () => Date.now();
/** A session that is gone or not the caller's. */
export const sessionNotFound = () => fault('not_found', 'Session not found', Status.NOT_FOUND);
/** Refusal for a project that was deleted. */
export const projectDeleted = () => fault('project_deleted', 'Project has been deleted', Status.GONE);
/** Refusal when a project or persona has no free slot. */
export const capacityReached = () =>
  fault('quota_exceeded', 'Browser or persona capacity reached', Status.TOO_MANY_REQUESTS);
/** Move a session to `next` and record it as an event; a no-op when it is already there. */
export function moveTo(tx, x, next) {
  if (x.state === next) return;
  x.state = next;
  tx.emit(x.project, `session.${next}`, x.id);
}
