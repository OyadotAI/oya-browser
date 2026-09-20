/**
 * The tick's transaction: renew gateway leases, move sessions whose lease lapsed
 * to their next state, and claim the ones due for provider cleanup.
 */
import { instanceId, attachOnly, live } from '../service.ts';
import { registry } from '../../browsers/registry.ts';
import { sessions as gateways } from '../../gateway/service.ts';
import {
  ATTACHMENT_LEASE_MS,
  ATTACHMENT_RENEW_BEFORE_MS,
  CLEANUP_BATCH,
  CLEANUP_LEASE_MS,
  HOLD_LEASE_MS,
  HOLD_RENEW_BEFORE_MS,
} from './constants.ts';

/** One pass over live sessions; returns the claimed cleanup jobs and a summary of every live session. */
export async function sweep(tx, now) {
  await renewGatewayLeases(tx, now);
  const sessions = await tx.list('session', { states: live }),
    jobs = [];
  for (const x of sessions) {
    expire(tx, x, now);
    if (claimable(x, now, jobs.length)) jobs.push(claim(x, now));
  }
  return { jobs, sessions: sessions.map(summary) };
}

/** Renew well before expiry so an idle tick commits nothing; expired rows are pruned by maintenance. */
async function renewGatewayLeases(tx, now) {
  for (const a of await tx.list('attachment', { states: [instanceId] }))
    if (gateways.has(a.id) && a.leaseUntil < now + ATTACHMENT_RENEW_BEFORE_MS) a.leaseUntil = now + ATTACHMENT_LEASE_MS;
  for (const h of await tx.list('hold'))
    if (gateways.has(h.sessionId) && h.expiresAt < now + HOLD_RENEW_BEFORE_MS) h.expiresAt = now + HOLD_LEASE_MS;
}

/** Moves a session whose lease lapsed to where it goes next, and emits the change. */
function expire(tx, x, now) {
  const prior = x.state,
    attached = registry.get(x.id) || gateways.get(x.id);
  // A lost attachment cannot return under its ID; a dial-in browser can.
  if (x.state === 'ready' && !attached && x.leaseUntil < now) x.state = lostState(x);
  if (['provisioning', 'unknown_outcome'].includes(x.state) && x.leaseUntil < now)
    x.state = x.cleanup ? 'cleanup_pending' : attachOnly.has(x.provider) ? 'failed' : 'unknown_outcome';
  if (x.state !== prior) tx.emit(x.project, `session.${x.state}`, x.id);
}

/** Where a ready session goes when its attachment is lost. */
function lostState(x) {
  if (x.cleanup) return 'cleanup_pending';
  return ['cdp', 'gateway'].includes(x.provider) ? 'stopped' : 'disconnected';
}

/** Whether this replica should take the session's cleanup now, given how many it has claimed this tick. */
function claimable(x, now, claimed) {
  if (claimed >= CLEANUP_BATCH || x.state !== 'cleanup_pending') return false;
  if (x.nextCleanupAt > now || x.cleanupLease > now || (x.provisioningActive && x.leaseUntil > now)) return false;
  // The live owner closes its own connection; another replica takes over once that lease lapses.
  return !(x.instance !== instanceId && x.leaseUntil > now);
}

/** Leases the cleanup to this replica and fences out older writers; returns a copy to work from. */
function claim(x, now) {
  x.cleanupLease = now + CLEANUP_LEASE_MS;
  x.fence += 1;
  return { ...x };
}

/** The fields the rest of the tick reads from a live session. */
const summary = ({ id, project, state, updatedAt, rateUsdHour }) => ({ id, project, state, updatedAt, rateUsdHour });
