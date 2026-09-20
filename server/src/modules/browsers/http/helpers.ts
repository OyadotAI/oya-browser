/**
 * Small pieces the browser and pool routes share.
 */
import { INTERNAL_ACTIONS, getKey } from '../../../app/http.ts';
import { audit } from '../../../platform/audit.ts';
import { Status } from '../../../platform/http-status.ts';

/** Long-running work (a navigate can take 90s): no socket timeout for this request. */
export function noTimeouts(req, res) {
  req.setTimeout(0);
  res.setTimeout(0);
}

/** Answers 400 for a missing action and 403 for a server-internal one; truthy when it answered. */
export function refuseAction(res, action) {
  if (!action) return res.status(Status.BAD_REQUEST).json({ error: 'Missing action' });
  if (INTERNAL_ACTIONS.has(action))
    return res.status(Status.FORBIDDEN).json({ error: `${action} is not available through this API` });
  return null;
}

/** Audits `action` on one browser by the caller; `outcome` defaults to ok. */
export function auditBrowser(req, action: string, browserId: string, meta: object, outcome?: string) {
  audit({ action, actorKey: getKey(req), targetType: 'browser', targetId: browserId, outcome, meta, req });
}
