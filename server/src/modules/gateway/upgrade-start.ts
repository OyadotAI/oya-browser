/**
 * A new session: lock the named profile, check quota, reserve with the
 * control plane, acquire a browser from the provider pool, then restore the
 * profile and start recording before the client is let in.
 */
import { control } from '../control/service.ts';
import { registry } from '../browsers/registry.ts';
import { metrics } from '../../platform/metrics.ts';
import { fingerprint } from '../../platform/audit.ts';
import * as usage from '../../platform/usage.ts';
import { checkQuota } from '../../platform/limits.ts';
import { Status } from '../../platform/http-status.ts';
import * as profiles from './profiles.ts';
import * as recorder from './recorder.ts';
import { sessions } from './session-store.ts';
import { acquireBrowser, markReady, unlockProfile, type Start } from './upgrade-acquire.ts';
import { Session } from './session.ts';
import { accept, auditAs, refuse, type Upgrade } from './upgrade-context.ts';

/** Whether this replica or the whole control plane is draining for a restart. */
async function draining() {
  return registry.draining || (await control().store.get('meta', 'draining'))?.value;
}

/** Starts a new session for the caller. */
export async function startSession(ctx: Upgrade) {
  if (await draining()) return ctx.deny(Status.UNAVAILABLE, 'Server draining');
  const start: Start = { ...ctx, profileName: ctx.url.searchParams.get('profile'), owner: fingerprint(ctx.token) };
  if (!lockProfile(start) || !withinQuota(start)) return;
  const session = await launch(start);
  if (session) accept(start, (client) => started(start, session, client));
}

/**
 * Two browsers sharing one jar corrupts it, so the second is refused
 * rather than silently racing.
 */
function lockProfile(start: Start) {
  if (!start.profileName || profiles.tryLock(start.owner, start.profileName).ok) return true;
  metrics.gatewayConnects.inc({ outcome: 'profile_busy' });
  auditProfileBusy(start);
  start.deny(Status.CONFLICT, 'Profile In Use');
  return false;
}

/** The audit record of a connect refused because the profile is in use. */
function auditProfileBusy(start: Start) {
  const meta = { reason: 'profile in use' };
  auditAs(start, {
    action: 'gateway.connect',
    outcome: 'denied',
    targetType: 'profile',
    targetId: start.profileName,
    meta,
  });
}

/** Whether the key may run another browser; refused with a 429 when not. */
function withinQuota(start: Start) {
  const mine = [...sessions.values()].filter((s) => s.apiKey === start.token).length;
  const quota = checkQuota('browsers', start.token, mine);
  if (quota.allowed) return true;
  unlockProfile(start);
  refuse(start, 'quota', Status.TOO_MANY_REQUESTS, `Browser quota reached (${quota.quota})`);
  return false;
}

/** Acquires the browser and opens the session on it; null once the caller has been refused. */
async function launch(start: Start) {
  const acquired = await acquireBrowser(start);
  if (!acquired || !(await markReady(start, acquired))) return null;
  const session = openSession(start, acquired);
  return (await prepare(start, session)) ? session : null;
}

/** The live session over the acquired browser. */
function openSession(start: Start, acquired) {
  const session = newSession(start, acquired);
  session.authToken = start.authToken;
  session.upstreamUrl = acquired.session.target.wsUrl;
  session.bindUpstream();
  sessions.set(session.id, session);
  return session;
}

/** The session object; its release hands the browser and the provider slot back and unlocks the profile. */
function newSession(start: Start, acquired) {
  return new Session({
    id: start.reservation.id,
    apiKey: start.token,
    provider: acquired.provider.name,
    profile: start.profileName || null,
    upstream: acquired.session.upstream,
    release: () => releaseAll(start, acquired),
  });
}

/** Gives back the browser, then the pool slot, then the profile lock. */
async function releaseAll(start: Start, acquired) {
  await acquired.session.target.release?.();
  await acquired.release();
  unlockProfile(start);
}

/**
 * Restore before the client can navigate, so the first page load already has
 * the profile's cookies; then start recording if asked. False once refused.
 */
async function prepare(start: Start, session) {
  if (start.profileName)
    await profiles
      .restore(start.owner, start.profileName, session)
      .catch((e) => console.error('[gateway] profile restore:', e.message));
  if (start.url.searchParams.get('record') !== '1') return true;
  return startRecording(start, session);
}

/** Starts recording; a session that cannot be recorded is ended and refused. */
async function startRecording(start: Start, session) {
  try {
    await recorder.start(session);
    return true;
  } catch (e) {
    await session.destroy('Recording unavailable');
    start.deny(e.status || Status.UNAVAILABLE, 'Recording unavailable');
    return false;
  }
}

/** The client is on its new browser. */
function started(start: Start, session, client) {
  session.attach(client);
  usage.record(start.token, 'browsers_started');
  metrics.gatewayConnects.inc({ outcome: 'ok' });
  metrics.gatewaySessions.set({}, sessions.size);
  auditStart(start, session);
}

/** The audit record of a session starting. */
function auditStart(start: Start, session) {
  const meta = {
    provider: session.provider,
    profile: start.profileName || null,
    recording: start.url.searchParams.get('record') === '1',
  };
  auditAs(start, { action: 'gateway.session.start', targetType: 'session', targetId: session.id, meta });
}
