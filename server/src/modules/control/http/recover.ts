/**
 * POST /control/sessions/:id/recover: the session itself if still ready;
 * otherwise, with `replace: true`, a replacement started from the same profile
 * once the original is stopped or failed.
 */
import { control, fault } from '../service.ts';
import { Status } from '../../../platform/http-status.ts';
import { key, redispatchStart } from './guards.ts';

/** Why recovery needs an explicit replacement. */
const REPLACEMENT_REQUIRED =
  'Original browser is unavailable. Explicit replacement is required; prior command outcomes may be unknown';

/** Answers with the original session, or starts its replacement through POST /browsers/start. */
export async function recover(req, res) {
  const x = await control().session(key(req), req.params.id);
  if (x.state === 'ready') return res.json({ outcome: 'original_session', session: x });
  assertReplaceable(req, x);
  req.recoveryPolicies = x.policies || [];
  req.body = replacementRequest(req, x);
  markRecovery(res, x);
  await redispatchStart(req, res, 'Replacement failed');
}

/** Refuses unless the caller asked for a replacement, the original has ended, and a CDP replacement names its endpoint. */
function assertReplaceable(req, x) {
  if (req.body?.replace !== true) throw fault('recovery_unavailable', REPLACEMENT_REQUIRED, Status.CONFLICT);
  if (!['stopped', 'failed'].includes(x.state))
    throw fault('cleanup_required', 'Stop the original resource and confirm cleanup before replacing it');
  if (x.provider === 'cdp' && !req.body?.wsUrl)
    throw fault('replacement_endpoint_required', 'Supply wsUrl for a replacement CDP browser', Status.UNPROCESSABLE);
}

/** The start request for a replacement of `x`. */
const replacementRequest = (req, x) => ({
  provider: x.provider,
  persona: x.persona,
  governed: !!x.runtime,
  replacementOf: x.id,
  ...(req.body?.wsUrl ? { wsUrl: req.body.wsUrl } : {}),
  ...(x.budgetUsd != null ? { budgetUsd: x.budgetUsd } : {}),
});

/** Adds the recovery outcome to whatever the start handler answers. */
function markRecovery(res, x) {
  const json = res.json.bind(res);
  res.json = (value) =>
    json({
      ...value,
      recovery: { outcome: 'replacement_from_profile', previousSessionId: x.id, previousCommandOutcome: 'unknown' },
    });
}
