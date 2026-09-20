/**
 * Ending a gateway session: release its command slots, save what it leaves
 * behind (attachment record, recording, profile), close its sockets, hand the
 * browser back and record what it used.
 */
import { control } from '../control/service.ts';
import { metrics } from '../../platform/metrics.ts';
import { audit } from '../../platform/audit.ts';
import * as usage from '../../platform/usage.ts';
import * as profiles from './profiles.ts';
import * as recorder from './recorder.ts';
import { sessions } from './session-store.ts';
import { CloseCode, MS_PER_SECOND } from './constants.ts';

/** Runs every teardown step for a session that has just been marked closed. */
export async function teardown(session, reason) {
  await session.commands.releaseAll();
  clearTimeout(session.graceTimer);
  sessions.delete(session.id);
  await saveLeftovers(session);
  closeSockets(session, reason);
  await returnBrowser(session);
  report(session, reason);
}

/** What outlives the session: the attachment record is dropped, the recording and profile saved. */
async function saveLeftovers(session) {
  await dropAttachment(session);
  await finishRecording(session);
  await captureProfile(session);
}

/** Removes the attachment record so the fleet browser is no longer counted as attached. */
async function dropAttachment(session) {
  if (!session.attachedTo) return;
  await control()
    .store.transact(async (tx) => {
      if (await tx.get('attachment', session.id)) await tx.delete('attachment', session.id);
    })
    .catch(() => {});
}

/** Stops the recording, announcing it when there was one. */
async function finishRecording(session) {
  if (await recorder.stop(session.id).catch(() => false))
    void control()
      .emit(session.apiKey, 'recording.ready', session.id, {})
      .catch(() => {});
}

/** Saves the profile's cookies and storage, then closes the connection restore held open. */
async function captureProfile(session) {
  if (session.profile) {
    await profiles
      .capture(session.owner, session.profile, session)
      .catch((e) => console.error(`[gateway] profile capture failed for ${session.profile}:`, e.message));
  }
  // Held open since restore so its on-new-document hook stays registered.
  try {
    session.profileConn?.close();
  } catch {}
}

/** Closes the client and the browser connection. */
function closeSockets(session, reason) {
  try {
    session.client?.close(CloseCode.GOING_AWAY, reason);
  } catch {}
  try {
    session.upstream?.close();
  } catch {}
}

/** Hands the browser back; a provider session is marked cleanup_pending until that succeeds, then stopped. */
async function returnBrowser(session) {
  if (!session.attachedTo) await setState(session, 'cleanup_pending').catch(() => {});
  try {
    await session.release?.();
    if (!session.attachedTo) await setState(session, 'stopped');
  } catch (err) {
    console.error('[gateway] cleanup pending:', err.message);
  }
}

/** Records the session's lifecycle state in the control plane. */
function setState(session, state) {
  return control().update(session.apiKey, session.id, { state });
}

/** Usage, metrics and the audit record for the ended session. */
function report(session, reason) {
  const seconds = session.elapsedSeconds();
  if (!session.attachedTo) usage.record(session.apiKey, 'browser_seconds', seconds);
  usage.record(session.apiKey, 'bytes_out', session.bytesDown);
  metrics.gatewaySessions.set({}, sessions.size);
  metrics.gatewaySessionDuration.observe({ provider: session.provider }, seconds * MS_PER_SECOND);
  auditEnd(session, seconds, reason);
}

/** The audit record of a session ending. */
function auditEnd(session, seconds, reason) {
  audit({
    action: 'gateway.session.end',
    actorKey: session.apiKey,
    targetType: 'session',
    targetId: session.id,
    meta: { provider: session.provider, seconds, reason, profile: session.profile || null },
  });
}
