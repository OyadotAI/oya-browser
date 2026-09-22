/**
 * Attaching a CDP browser: one we dial out to, rather than one that dials in.
 * Anchor, Browserbase, Steel, or any Chrome with --remote-debugging-port.
 */
import { randomUUID } from 'crypto';
import { CDPDriver } from '../../../drivers/cdp.ts';
import { metrics } from '../../../platform/metrics.ts';
import { audit } from '../../../platform/audit.ts';
import { Status } from '../../../platform/http-status.ts';
import { getKey } from '../../../app/http.ts';
import * as usage from '../../../platform/usage.ts';
import { registry } from '../registry.ts';
import { browserQuota, countBrowsers, forget, quotaBody, refuseBadName } from '../lifecycle/fleet.ts';
import { acquireSession, addCdpBrowser, claimSession, refusal, releaseQuietly } from '../lifecycle/sessions.ts';

/** Attaches the caller's CDP browser; answers 201, 400 for a bad name, 429 over quota, or the failure. */
export async function connectCdp(req, res) {
  const key = getKey(req);
  if (refuseBadName(res, req.body?.name) || refuseOverQuota(req, res, key)) return;
  const provider = String(req.body?.provider || 'cdp');
  const browserId = req.controlSession?.id || randomUUID();
  const acquired = await acquire(req, res, key, provider, browserId);
  if (!acquired) return;
  await attach(req, res, { key, provider, session: acquired.session, browserId });
}

/** Answers 429 when the key is at its browser quota; truthy when it answered. */
function refuseOverQuota(req, res, key) {
  const quota = browserQuota(key);
  if (!quota.allowed) overQuota(req, res, key, quota);
  return !quota.allowed;
}

/** Audits a browser.connect with the given fields. */
function auditConnect(req, key, fields: object) {
  audit({ action: 'browser.connect', actorKey: key, targetType: 'browser', ...fields, req });
}

/** The key is at its browser quota: audited as denied, answered 429. */
function overQuota(req, res, key, quota) {
  auditConnect(req, key, { outcome: 'denied', meta: { reason: 'quota', quota: quota.quota } });
  return res.status(Status.TOO_MANY_REQUESTS).json(quotaBody(quota));
}

/** The vendor session as `{ session }`, or null once the failure is audited and answered with its status and code. */
async function acquire(req, res, key, provider, browserId) {
  try {
    return { session: await acquireSession(req, key, provider, req.body?.wsUrl, browserId) };
  } catch (err) {
    auditConnect(req, key, { outcome: 'error', meta: { provider, error: err.message } });
    res.status(err.status || Status.BAD_GATEWAY).json(refusal(err));
    return null;
  }
}

/**
 * Drives the session and registers the browser. On failure the session is handed
 * back, and so is a driver that had already connected: its socket to the
 * caller's browser would otherwise stay open with nothing holding it.
 */
async function attach(req, res, target) {
  const held: Held = { driver: null };
  try {
    await register(req, res, target, held);
  } catch (err) {
    letGoQuietly(target, held.driver);
    await attachFailed(req, res, target, err);
  }
}

/** What an attach holds so far, so a failure knows what to let go of. */
type Held = {
  /** The driver, once it has connected. */
  driver: CDPDriver | null;
};

/** Claims the session, connects a driver, registers the browser and answers 201. */
async function register(req, res, { key, session, browserId }, held: Held) {
  await claimSession(key, browserId, session);
  held.driver = await new CDPDriver(plainDriver(session, browserId)).connect();
  addCdpBrowser(req, key, browserId, session, { engine: held.driver, release: session.release });
  attached(req, res, key, browserId, session);
}

/**
 * Undoes what an attach got as far as: a registered browser is removed, which
 * closes its driver, and its usage clock stops; else a connected driver is
 * closed here. An undo that throws is logged, never allowed to leave the
 * caller with no answer.
 */
function letGoQuietly({ key, browserId }, driver: CDPDriver | null) {
  try {
    if (registry.isConnected(browserId)) return dropRegistered(key, browserId);
    driver?.close();
  } catch (err) {
    console.error('[connect] undo:', err.message);
  }
}

/** Removes a browser that got as far as the registry, and ends its usage clock. */
function dropRegistered(key, browserId) {
  registry.remove(browserId);
  usage.browserDisconnected(key, browserId);
  countBrowsers();
}

/** A driver with no persona: the browser keeps whatever identity it already has. */
const plainDriver = (session, browserId) => ({
  wsUrl: session.wsUrl,
  provider: session.provider,
  onClose: () => forget(browserId),
});

/** Books the attach and answers 201. */
function attached(req, res, key, browserId, session) {
  metrics.wsConnections.inc({ outcome: 'ok' });
  countBrowsers();
  const meta = { provider: session.provider, sessionId: session.sessionId };
  auditConnect(req, key, { targetId: browserId, meta });
  res.status(Status.CREATED).json({ id: browserId, provider: session.provider, clientType: 'cdp' });
}

/** Hands the session back, audits the failure and answers 502. */
async function attachFailed(req, res, { key, provider, session }, err) {
  await releaseQuietly(session);
  auditConnect(req, key, { outcome: 'error', meta: { provider, error: err.message } });
  const error = `Could not attach to the CDP browser: ${err.message}`;
  res.status(Status.BAD_GATEWAY).json({ error, code: 'provider_failed' });
}
