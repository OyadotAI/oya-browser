/**
 * The line between what an agent's own key gets for free and what needs a
 * person. Browsers the caller brings (the desktop app, a CDP URL, a vendor on
 * the caller's own account) cost this server nothing; browsers it runs itself
 * cost money, so an unclaimed agent key is sent to be claimed first.
 */
import { getKey } from '../../../app/http.ts';
import { Status } from '../../../platform/http-status.ts';
import * as keyConfig from '../../config/service.ts';
import { claimUrl, isUnclaimedAgentKey } from '../../auth/service.ts';
import { track } from '../../telemetry/index.ts';

/** Providers whose browsers this server runs, and pays for. */
const SERVER_RUN = new Set(['oya-cloud', 'oya-selfhosted']);

/** The 403 an unclaimed agent key gets, with the link its person opens to claim it. */
const claimFirst = (key: string) => ({
  error: 'Oya Cloud browsers need a person: have them open claim_url while signed in. The desktop app works now.',
  code: 'claim_required',
  claim_url: claimUrl(key),
});

/** Refuses a server-run browser to an unclaimed agent key; `fixed` names the provider when the route decides it. */
export function cloudNeedsPerson(fixed?: string) {
  return async (req, res, next) => {
    const key = getKey(req);
    const provider = fixed || String(req.body?.provider || keyConfig.providerFor(key));
    if (!SERVER_RUN.has(provider) || !(await isUnclaimedAgentKey(key))) return next();
    track.agentCloudRefused(key, provider);
    res.status(Status.FORBIDDEN).json(claimFirst(key));
  };
}
