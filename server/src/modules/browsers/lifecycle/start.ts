/**
 * Starting a browser: the one endpoint a developer needs. Which provider runs
 * the browser is configuration, not the caller's problem, /browsers/connect
 * and /browsers/provision remain as the explicit escape hatches.
 */
import { getKey } from '../../../app/http.ts';
import { audit } from '../../../platform/audit.ts';
import { Status } from '../../../platform/http-status.ts';
import * as keyConfig from '../../config/service.ts';
import { control } from '../../control/service.ts';
import { managedConfigured } from '../../control/managed.ts';
import { browserQuota, quotaBody, refuseBadName } from './fleet.ts';
import { resolvePersona } from './persona.ts';
import { refusal } from './sessions.ts';
import { launchCdp } from './start-cdp.ts';
import { launchManaged, launchSandbox } from './start-oya.ts';
import { nothingToDial, startWithConnected } from './start-connected.ts';
import type { Start } from './start-reply.ts';

/** Starts a browser on whichever provider the request or the key's configuration names. */
export async function startBrowser(req, res) {
  const key = getKey(req);
  if (refuseBadName(res, req.body?.name)) return;
  const wanted = String(req.body?.provider || keyConfig.providerFor(key));
  const resolved = resolvePersona(res, key, req.body?.profile || req.body?.persona, Status.BAD_REQUEST);
  if (!resolved || overQuota(res, key)) return;
  await launch({ req, res, key, wanted, persona: resolved.persona });
}

/** Answers 429 when the key holds its quota of browsers already. */
function overQuota(res, key) {
  const quota = browserQuota(key);
  if (!quota.allowed) res.status(Status.TOO_MANY_REQUESTS).json(quotaBody(quota));
  return !quota.allowed;
}

/** Runs the launcher; any failure is audited and answered with its status and code, else 502. */
async function launch(start: Start) {
  try {
    await control().assertProvisioning(start.key, start.req.controlSession.id);
    await launcherFor(start)(start);
  } catch (err) {
    const meta = { provider: start.wanted, error: err.message };
    audit({ action: 'browser.start', actorKey: start.key, outcome: 'error', meta, req: start.req });
    start.res.status(err.status || Status.BAD_GATEWAY).json(refusal(err));
  }
}

/** A self-hosted browser under governance runs in the managed runtime. */
const wantsManaged = (req) =>
  managedConfigured() ||
  req.controlSession?.runtimeRequired ||
  req.controlSession?.policies?.length ||
  req.body?.governed;

/** Managed, sandbox, the browser already connected, or a CDP browser we dial out to. */
function launcherFor({ req, key, wanted }: Start) {
  if (nothingToDial(req, key, wanted)) return startWithConnected;
  if (wanted === 'oya-selfhosted' && wantsManaged(req)) return launchManaged;
  if (wanted === 'oya-cloud' || wanted === 'oya-selfhosted') return launchSandbox;
  return launchCdp;
}
