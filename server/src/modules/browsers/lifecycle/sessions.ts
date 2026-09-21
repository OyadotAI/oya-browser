/**
 * Hosted CDP sessions: acquiring one from its vendor, tying it to the durable
 * session, handing it back, and registering the browser it drives.
 */
import { registry } from '../registry.ts';
import * as usage from '../../../platform/usage.ts';
import { acquire as acquireBrowser } from '../../../drivers/providers.ts';
import { assertSafeTarget } from '../../../platform/net-guard.ts';
import * as keyConfig from '../../config/service.ts';
import { control } from '../../control/service.ts';
import { displayName } from './fleet.ts';

/** Acquires a browser from `provider`; a raw CDP address passes the network guard first. */
export async function acquireSession(req, key, provider, wsUrl) {
  // http and https are here because Chrome prints its debugging port that way;
  // providers.ts resolves it through /json/version on the same host.
  const protocols = ['ws:', 'wss:', 'http:', 'https:'];
  if (provider === 'cdp' && wsUrl) await assertSafeTarget(wsUrl, { protocols, label: 'wsUrl' });
  const onCreated = (cleanup) => control().update(key, req.controlSession.id, { cleanup });
  return acquireBrowser({ provider, wsUrl, env: keyConfig.envFor(key), onCreated });
}

/** Records the vendor's cleanup on the durable session, then checks provisioning may go on. */
export async function claimSession(key, browserId, session) {
  if (session.cleanup) await control().update(key, browserId, { cleanup: session.cleanup });
  await control().assertProvisioning(key, browserId);
}

/** Hands a vendor session back; a failure is logged, never thrown. */
export function releaseQuietly(session) {
  return session.release().catch((err) => console.error('[providers] cleanup:', err.message));
}

/** Registers a CDP browser this server dialled, and counts it for usage. */
export function addCdpBrowser(req, key, browserId, session, extra: object) {
  const name = displayName(req, session);
  registry.add(browserId, { apiKey: key, name, clientType: 'cdp', provider: session.provider, ...extra });
  usage.browserConnected(key, browserId);
}
