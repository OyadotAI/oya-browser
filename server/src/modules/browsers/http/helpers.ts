/**
 * Small pieces the browser and pool routes share.
 */
import { INTERNAL_ACTIONS, getKey } from '../../../app/http.ts';
import { audit } from '../../../platform/audit.ts';
import { Status } from '../../../platform/http-status.ts';
import { HttpError, invalid, sendError } from '../../../platform/errors.ts';

/** Long-running work (a navigate can take 90s): no socket timeout for this request. */
export function noTimeouts(req, res) {
  req.setTimeout(0);
  res.setTimeout(0);
}

/**
 * Answers 400 for a missing action or one that is not a string, and 403 for a
 * server-internal one; truthy when it answered. An action is JSON from the
 * caller: `["evaluate_raw"]` is not in the internal set, yet the browser's
 * command maps would read it as the string, so only a string goes further.
 */
export function refuseAction(res, action) {
  if (!action) return sendError(res, new HttpError(Status.BAD_REQUEST, 'Missing action', { field: 'action' }));
  if (typeof action !== 'string') return sendError(res, invalid('action', 'a string', action));
  if (INTERNAL_ACTIONS.has(action)) return sendError(res, internalAction(action));
  return null;
}

/** An action only the server sends, refused to a caller in the API's one error shape. */
const internalAction = (action: string) =>
  new HttpError(Status.FORBIDDEN, `${action} is not available through this API`, { code: 'action_forbidden', action });

/** Audits `action` on one browser by the caller; `outcome` defaults to ok. */
export function auditBrowser(req, action: string, browserId: string, meta: object, outcome?: string) {
  audit({ action, actorKey: getKey(req), targetType: 'browser', targetId: browserId, outcome, meta, req });
}
