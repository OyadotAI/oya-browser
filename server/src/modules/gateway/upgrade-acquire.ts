/**
 * Getting a browser for a new session: a control-plane reservation, then a
 * provider from the pool, dialled and marked ready. Every failure answers the
 * caller and gives back whatever was already taken.
 */
import { control } from '../control/service.ts';
import * as keyConfig from '../config/service.ts';
import { acquire as acquireProvider } from '../../drivers/providers.ts';
import { metrics } from '../../platform/metrics.ts';
import { QUOTAS } from '../../platform/limits.ts';
import { Status } from '../../platform/http-status.ts';
import * as profiles from './profiles.ts';
import { pool } from './routing.ts';
import { dial, opened } from './upstream.ts';
import { auditAs, denyUpstream, type Upgrade } from './upgrade-context.ts';
import { SERVER_ERROR_MIN } from './constants.ts';

/** A new-session upgrade: the caller plus the profile it names and, once made, its reservation. */
export interface Start extends Upgrade {
  /** The ?profile= to restore and capture, or null. */
  profileName: string | null;
  /** Fingerprint of the caller's key; profiles and providers are namespaced by it. */
  owner: string;
  /** The control-plane reservation, once made; its id becomes the session id. */
  reservation?: any;
}

/** Releases the profile lock, if one was taken. */
export function unlockProfile(start: Start) {
  if (start.profileName) profiles.unlock(start.owner, start.profileName);
}

/** Reserves and acquires a browser; null once the caller has been refused. */
export async function acquireBrowser(start: Start) {
  try {
    start.reservation = await reserve(start);
    return await acquireFrom(start);
  } catch (err) {
    unlockProfile(start);
    return start.reservation ? providerFailed(start, err) : admissionRefused(start, err);
  }
}

/** The control-plane reservation: admission, quota and the profile's persona limit. */
function reserve(start: Start) {
  return control().reserve(start.token, {
    provider: 'gateway',
    maxConcurrent: QUOTAS.browsers,
    request: { profile: start.profileName },
    persona: start.profileName ? `profile:${start.profileName}` : null,
    personaLimit: 1,
  });
}

/** A provider slot with a connected browser. */
function acquireFrom(start: Start) {
  return pool.acquire({
    // This key's own providers plus whatever the host shares.
    owner: start.owner,
    strategy: start.url.searchParams.get('strategy') || undefined,
    connect: (provider) => connectProvider(start, provider),
  });
}

/** Admission refusals (quota, drain, policy) are the caller's answer, not a provider failure. */
function admissionRefused(start: Start, err) {
  metrics.gatewayConnects.inc({ outcome: 'quota' });
  start.deny(
    err.status || Status.UNAVAILABLE,
    err.status && err.status < SERVER_ERROR_MIN ? err.message : 'Service Unavailable',
  );
  return null;
}

/** No provider could serve: close the reservation and answer 503 or 502. */
async function providerFailed(start: Start, err) {
  await closeReservation(start, err);
  metrics.gatewayConnects.inc({ outcome: 'no_provider' });
  auditAs(start, { action: 'gateway.connect', outcome: 'error', meta: { error: err.message } });
  denyUpstream(start, err.status);
  return null;
}

/** Completes the reservation as failed; a storage error here is not the caller's problem. */
async function closeReservation(start: Start, err) {
  const error = { error: 'Provider acquisition failed' };
  await control()
    .complete(start.token, start.reservation.id, err.status || Status.BAD_GATEWAY, error)
    .catch(() => {});
}

/** Gets a browser from the provider and opens its CDP socket. */
async function connectProvider(start: Start, provider) {
  return dialTarget(await targetFor(start, provider));
}

/** A plain CDP provider is dialled directly; any other is asked for a browser. */
function targetFor(start: Start, provider) {
  if (provider.type === 'cdp' && provider.wsUrl)
    return { wsUrl: provider.wsUrl, provider: provider.name, sessionId: null, release: async () => {} };
  return acquireProvider({
    provider: provider.type,
    env: provider.owner === null ? process.env : keyConfig.envFor(start.token),
    onCreated: (cleanup) => control().update(start.token, start.reservation.id, { cleanup }),
  });
}

/** Opens the browser's socket; on failure the browser is handed back. */
async function dialTarget(target) {
  let upstream;
  try {
    upstream = dial(target.wsUrl);
    await opened(upstream);
    return { upstream, target };
  } catch {
    return abandon(upstream, target);
  }
}

/** Drops a browser that could not be reached. */
async function abandon(upstream, target): Promise<never> {
  upstream?.terminate();
  await target.release().catch((err) => console.error('[gateway] cleanup:', err.message));
  throw new Error('Provider browser connection failed');
}

/** Links the provider hold to the session and marks it ready; false once refused. */
export async function markReady(start: Start, acquired) {
  try {
    await linkHold(acquired.holdId, start.reservation.id);
    await control().update(start.token, start.reservation.id, readyState(acquired));
    return true;
  } catch (err) {
    await giveBack(start, acquired, err);
    return false;
  }
}

/** Points the provider hold at the session, so the worker renews it while the session lives. */
async function linkHold(holdId, id) {
  await control().store.transact(async (tx) => {
    const hold = await tx.get('hold', holdId);
    if (hold) hold.sessionId = id;
  });
}

/** provider stays 'gateway' so lifecycle rules still know it is attach-only; vendor records where it came from. */
function readyState(acquired) {
  return {
    state: 'ready',
    provisioningActive: false,
    vendor: acquired.provider.name,
    cleanup: acquired.session.target.cleanup || null,
  };
}

/** Stopped while acquiring, or storage lost: hand the browser back rather than leak it. */
async function giveBack(start: Start, acquired, err) {
  acquired.session.upstream.terminate();
  await Promise.allSettled([acquired.session.target.release?.(), acquired.release()]);
  unlockProfile(start);
  if (err.status === Status.CONFLICT) start.deny(Status.CONFLICT, 'Conflict');
  else start.deny(Status.UNAVAILABLE, 'Service Unavailable');
}
