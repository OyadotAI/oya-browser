/** Session helpers shared by the control operations: the public view, ownership, and stopping. */
import { projectId, sessionNotFound, stamp } from './model.ts';

/** A session without what only the server may see: its cleanup descriptor, provider response and sealed request. */
export const publicSession = ({ cleanup, response, requestHash, queuedRequest, egressHash, enrollmentHash, ...rest }) =>
  rest;

/** Stop a live session. Force is the operator's reconciliation: with no deletion descriptor and no creation in flight, they assert the resource is gone. */
export function stopSession(tx, x, { force = false, reason = 'cancelled' } = {}) {
  const reconcile = force && x.state !== 'queued' && !x.cleanup && !(x.provisioningActive && x.leaseUntil > stamp());
  x.state = x.state === 'queued' || reconcile ? 'stopped' : 'cleanup_pending';
  tx.emit(x.project, `session.${x.state}`, x.id, { reason: reconcile ? 'reconciled' : reason });
}

/** The session if it belongs to the key's project; 404 otherwise, so other projects' ids are not disclosed. */
export async function ownSession(tx, key, id) {
  const x = await tx.get('session', id);
  if (!x || x.project !== projectId(key)) throw sessionNotFound();
  return x;
}
