/**
 * Oya Cloud provisioning routes. Sandboxed browsers enroll over the normal
 * WebSocket with the caller's own key, so they join that caller's pool as
 * ordinary browsers.
 */
import { registry } from '../registry.ts';
import { isConfigured as sandboxConfigured, removeSandbox } from '../../../drivers/sandbox.ts';
import { metrics } from '../../../platform/metrics.ts';
import { audit } from '../../../platform/audit.ts';
import { checkHourly } from '../../../platform/limits.ts';
import { Status } from '../../../platform/http-status.ts';
import { getKey } from '../../../app/http.ts';
import { sandboxMissing } from '../lifecycle/sandbox-settings.ts';
import { auditProvision, bookProvision, createMany } from '../lifecycle/provision.ts';
import { MAX_NAME, MAX_PROVISION } from '../constants.ts';

/** What the caller is told about provisioned browsers. */
const JOIN_NOTE = 'Browsers connect on their own; they appear in GET /browsers within ~90s.';

/** Launches up to 100 sandboxes, unless draining, over the hourly quota, or unconfigured. */
export async function provision(req, res) {
  if (registry.draining) return res.status(Status.UNAVAILABLE).json({ error: 'Server is draining' });
  const hourly = checkHourly('sandboxesPerHour', getKey(req));
  if (!hourly.allowed) return hourlyDenied(req, res, hourly);
  if (!sandboxConfigured()) return sandboxMissing(res);
  await provisionFleet(req, res);
}

/** The key has launched its hourly quota of sandboxes: audited as denied, answered 429. */
function hourlyDenied(req, res, hourly) {
  const meta = { reason: 'hourly quota', quota: hourly.quota };
  audit({ action: 'browser.provision', actorKey: getKey(req), outcome: 'denied', meta, req });
  return res
    .status(Status.TOO_MANY_REQUESTS)
    .json({ error: `Sandbox quota reached for this hour (${hourly.quota})`, ...hourly });
}

/** Creates the sandboxes; 202 when any were created, else 502. */
async function provisionFleet(req, res) {
  const key = getKey(req);
  const count = Math.min(Math.max(parseInt(req.body?.count) || 1, 1), MAX_PROVISION);
  const name = typeof req.body?.name === 'string' ? req.body.name.slice(0, MAX_NAME) : undefined;
  const { created, failed } = await createMany(key, name, count);
  bookProvision(key, count, created, failed);
  auditProvision(req, key, count, created, failed);
  const body = { ok: created.length > 0, requested: count, browsers: created, failed, note: JOIN_NOTE };
  res.status(created.length ? Status.ACCEPTED : Status.BAD_GATEWAY).json(body);
}

/** Destroys a browser's sandbox; ownership is checked by its owner label. */
export async function deleteSandbox(req, res) {
  try {
    res.json({ ok: true, removed: await removeOwned(req, req.params.browserId) });
  } catch (err) {
    console.error('[sandbox] delete failed:', err.message);
    res.status(err.status || Status.BAD_GATEWAY).json({ error: err.message });
  }
}

/**
 * Ownership is enforced by the sandbox's owner label, which works whether or
 * not the browser is currently connected.
 */
async function removeOwned(req, browserId) {
  const removed = await removeSandbox(browserId, getKey(req));
  metrics.sandboxes.inc({ op: 'delete', outcome: 'ok' });
  const meta = { removed };
  audit({ action: 'sandbox.delete', actorKey: getKey(req), targetType: 'sandbox', targetId: browserId, meta, req });
  return removed;
}
