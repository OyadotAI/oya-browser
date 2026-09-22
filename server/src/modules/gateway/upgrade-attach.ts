/**
 * Attach to a browser already in the fleet.
 *
 * "Connect Playwright to *this* browser" from the console. The registry
 * browser stays where it is; this is a second CDP client on the same
 * upstream, which Chrome allows. An Oya client has no endpoint to dial, so its
 * CDP is relayed over the socket it dialled us on (cdp-relay.js), if it
 * enrolled with its front door on.
 */
import { randomUUID } from 'crypto';
import { track } from '../telemetry/index.ts';
import { control, projectId, instanceId } from '../control/service.ts';
import { registry } from '../browsers/registry.ts';
import { metrics } from '../../platform/metrics.ts';
import { Status } from '../../platform/http-status.ts';
import { Session } from './session.ts';
import { sessions } from './session-store.ts';
import { accept, auditAs, refuse, type Refusal, type Upgrade } from './upgrade-context.ts';
import { ATTACHMENT_LEASE_MS } from './constants.ts';

/**
 * An attached session lives only as long as its browser. A vendor's Chrome
 * keeps the second socket open after the registry lets the browser go, so
 * nothing would tell the client; ending the session here closes it with
 * "browser stopped", which is how Playwright learns the browser is gone.
 */
registry.on('browser:disconnected', ({ id }) => {
  for (const session of sessions.values()) if (session.attachedTo === id) void session.destroy('browser stopped');
});

/** Why this caller may not attach to the browser, or null when it may. */
function attachRefusal(target, token): Refusal | null {
  if (!target || target.apiKey !== token) return ['unknown_browser', Status.NOT_FOUND, 'Not Found'];
  if (!target.driver.cdpEndpoint()) return ['not_attachable', Status.CONFLICT, 'Not Attachable'];
  return null;
}

/** Opens a session on the fleet browser named by ?browser=. */
export async function attachToFleet(ctx: Upgrade) {
  const attachId = ctx.url.searchParams.get('browser');
  const target = registry.get(attachId);
  const refusal = attachRefusal(target, ctx.token);
  if (refusal) return refuse(ctx, ...refusal);
  const dialled = await connectTo(ctx, target, attachId);
  if (!dialled) return;
  const session = await attachedSession(ctx, target, attachId, dialled.upstream);
  accept(ctx, (client) => attached(ctx, session, target, attachId, client));
}

/** The browser's CDP socket, or its relay; null after answering 502. */
async function connectTo(ctx: Upgrade, target, attachId) {
  try {
    return { upstream: await target.driver.cdpEndpoint().open() };
  } catch (err) {
    metrics.gatewayConnects.inc({ outcome: 'attach_failed' });
    auditAttachFailure(ctx, attachId, err);
    ctx.deny(Status.BAD_GATEWAY, 'Bad Gateway');
    return null;
  }
}

/** The audit record of a failed attach. */
function auditAttachFailure(ctx: Upgrade, attachId, err) {
  const meta = { error: err.message };
  auditAs(ctx, { action: 'gateway.connect', outcome: 'error', targetType: 'browser', targetId: attachId, meta });
}

/** A live session on the fleet browser, recorded as an attachment. */
async function attachedSession(ctx: Upgrade, target, attachId, upstream) {
  const session = newAttachedSession(ctx, target, upstream);
  session.endpoint = target.driver.cdpEndpoint();
  session.attachedTo = attachId;
  await recordAttachment(session, ctx.token, attachId);
  session.authToken = ctx.authToken;
  session.bindUpstream();
  sessions.set(session.id, session);
  return session;
}

/** The session object; it has nothing to release, since the browser belongs to the registry and stays. */
function newAttachedSession(ctx: Upgrade, target, upstream) {
  return new Session({
    id: randomUUID(),
    apiKey: ctx.token,
    provider: target.provider || 'cdp',
    profile: null,
    upstream,
    release: () => {},
  });
}

/** Stores the attachment with a lease the control worker renews while the session lives. */
async function recordAttachment(session, token, attachId) {
  await control().store.transact(async (tx) => {
    tx.put('attachment', session.id, attachmentRecord(session, token, attachId));
  });
}

/** The attachment row. */
function attachmentRecord(session, token, attachId) {
  return {
    id: session.id,
    project: projectId(token),
    instance: instanceId,
    leaseUntil: Date.now() + ATTACHMENT_LEASE_MS,
    browserId: attachId,
  };
}

/** The client is on the fleet browser. */
function attached(ctx: Upgrade, session, target, attachId, client) {
  session.attach(client);
  metrics.gatewayConnects.inc({ outcome: 'attached' });
  metrics.gatewaySessions.set({}, sessions.size);
  const meta = { session: session.id, provider: target.provider };
  auditAs(ctx, { action: 'gateway.session.attach', targetType: 'browser', targetId: attachId, meta });
  track.cdpAttached(ctx.token, { provider: String(target.provider || 'oya') });
}
