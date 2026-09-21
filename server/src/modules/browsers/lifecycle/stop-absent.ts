/**
 * Stopping a browser this replica does not hold: a gateway session, a durable
 * session in the control plane, or an Oya Cloud sandbox that has not dialled in.
 */
import { isConfigured as sandboxConfigured, removeSandbox } from '../../../drivers/sandbox.ts';
import { killSession, sessions as gatewaySessions } from '../../gateway/service.ts';
import { control } from '../../control/service.ts';
import { auditStop, notConnected } from './stop-results.ts';

/** Durable states that are already over. */
const FINISHED = ['stopped', 'failed'];

/** Stops whichever of those `browserId` turns out to be. */
export async function stopAbsent(req, key, browserId, force) {
  if (gatewaySessions.get(browserId)?.apiKey === key) return killGateway(browserId);
  const durable = await control().findSession(key, browserId);
  if (durable) return stopDurable(key, browserId, durable, force);
  const removed = sandboxConfigured() && (await removeOrphan(req, key, browserId));
  return removed || notConnected(browserId);
}

/** Ends the caller's own gateway session. */
async function killGateway(browserId) {
  await killSession(browserId);
  return { id: browserId, ok: true };
}

/** Queued work stops at once; force reconciles a resource the system has no way to delete. */
async function stopDurable(key, browserId, durable, force) {
  if (FINISHED.includes(durable.state)) return { id: browserId, ok: true, status: durable.state };
  const { state } = await control().cancel(key, browserId, { force });
  return { id: browserId, ok: true, status: state };
}

/** Removes the key's sandbox by this name, if there is one; null when there is none. */
async function removeOrphan(req, key, browserId) {
  try {
    if (!(await removeSandbox(browserId, key))) return null;
    auditStop(req, key, browserId, { provider: 'oya-cloud', sandboxRemoved: true });
    return { id: browserId, ok: true, sandboxRemoved: true, provider: 'oya-cloud' };
  } catch (err) {
    return { id: browserId, ok: false, error: err.message };
  }
}
