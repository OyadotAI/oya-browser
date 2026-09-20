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
import { browserQuota, countBrowsers, forget, quotaBody } from '../lifecycle/fleet.ts';
import { acquireSession, addCdpBrowser, claimSession, releaseQuietly } from '../lifecycle/sessions.ts';

/** Attaches the caller's CDP browser; answers 201, 429 over quota, or the failure. */
export async function connectCdp(req, res) {
  const key = getKey(req);
  const quota = browserQuota(key);
  if (!quota.allowed) return overQuota(req, res, key, quota);
  const provider = String(req.body?.provider || 'cdp');
  const acquired = await acquire(req, res, key, provider);
  if (!acquired) return;
  const browserId = req.controlSession?.id || randomUUID();
  await attach(req, res, { key, provider, session: acquired.session, browserId });
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

/** The vendor session as `{ session }`, or null once the failure is audited and answered. */
async function acquire(req, res, key, provider) {
  try {
    return { session: await acquireSession(req, key, provider, req.body?.wsUrl) };
  } catch (err) {
    auditConnect(req, key, { outcome: 'error', meta: { provider, error: err.message } });
    res.status(err.status || Status.BAD_GATEWAY).json({ error: err.message });
    return null;
  }
}

/** Drives the session and registers the browser; on failure the session is handed back. */
async function attach(req, res, { key, provider, session, browserId }) {
  try {
    await claimSession(key, browserId, session);
    const driver = await new CDPDriver(plainDriver(session, browserId)).connect();
    addCdpBrowser(req, key, browserId, session, { driver, release: session.release });
    attached(req, res, key, browserId, session);
  } catch (err) {
    await attachFailed(req, res, { key, provider, session }, err);
  }
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
  res.status(Status.BAD_GATEWAY).json({ error: `Could not attach to the CDP browser: ${err.message}` });
}
