/**
 * Hosted CDP sessions: acquiring one from its vendor, reserving its endpoint
 * for one browser, tying it to the durable session, handing it back, and
 * registering the browser it drives.
 */
import { registry } from '../registry.ts';
import type { BrowserSpec } from '../registry/record.ts';
import * as usage from '../../../platform/usage.ts';
import { acquire as acquireBrowser } from '../../../drivers/providers.ts';
import { assertSafeTarget } from '../../../platform/net-guard.ts';
import { HttpError, answerFor } from '../../../platform/errors.ts';
import { Status } from '../../../platform/http-status.ts';
import * as keyConfig from '../../config/service.ts';
import { control } from '../../control/service.ts';
import { displayName } from './fleet.ts';

/** Who drives an endpoint: the browser, and the key it belongs to. */
type Holder = {
  /** The browser driving it. */
  browserId: string;
  /** The key that browser belongs to. */
  apiKey: string;
};

/**
 * Endpoints a live browser is driving, by their one spelling. Two browsers on
 * one Chrome would silently share its pages, so the second is refused. Held
 * from the moment the session is acquired, before any dial, and freed by the
 * session's release, which every way out of a start already calls.
 */
const held = new Map<string, Holder>();

/** A Chrome's own browser endpoint path: its id is random per Chrome process, so it names the Chrome itself. */
const CHROME_BROWSER_PATH = /^\/devtools\/browser\/[0-9a-f-]{36}$/i;

/**
 * One spelling for an endpoint. A Chrome's browser address is keyed by the id
 * in its path, since one Chrome answers to every name its host has
 * (localhost, 127.0.0.1, a LAN address). Anything else, a vendor's session
 * address, is keyed by the whole URL with the host lowercased.
 */
function endpointKey(wsUrl: string) {
  const url = new URL(wsUrl);
  return CHROME_BROWSER_PATH.test(url.pathname) ? `chrome:${url.pathname.toLowerCase()}` : url.href;
}

/** Where a person sees the endpoint: host and port only, never the path, which can carry a vendor's token. */
const hostOf = (wsUrl: string) => new URL(wsUrl).host;

/** The refusal for an endpoint already driven: the holder is named when it is the caller's, and only then. */
function inUse(wsUrl: string, holder: Holder, apiKey: string) {
  const where = `The Chrome at ${hostOf(wsUrl)} is already held by`;
  if (holder.apiKey !== apiKey) {
    const message = `${where} another browser. Stop it there, or start on another endpoint.`;
    return new HttpError(Status.CONFLICT, message, { code: 'endpoint_in_use' });
  }
  const message = `${where} browser ${holder.browserId}. Stop that browser, or drive it.`;
  return new HttpError(Status.CONFLICT, message, { code: 'endpoint_in_use', browserId: holder.browserId });
}

/** A release that first frees the endpoint, while this browser still holds it, then hands the session back. */
const freeing = (endpoint: string, browserId: string, release: () => Promise<void>) => async () => {
  if (held.get(endpoint)?.browserId === browserId) held.delete(endpoint);
  await release();
};

/**
 * Reserves the session's endpoint for `browserId`, or hands the session back
 * and refuses. Nothing is awaited between the lookup and the reservation, so
 * two starts racing on one endpoint cannot both pass.
 */
async function reserve(session, apiKey: string, browserId: string) {
  const endpoint = endpointKey(session.wsUrl);
  const holder = held.get(endpoint);
  if (holder) {
    await releaseQuietly(session);
    throw inUse(session.wsUrl, holder, apiKey);
  }
  held.set(endpoint, { browserId, apiKey });
  return { ...session, release: freeing(endpoint, browserId, session.release) };
}

/**
 * Acquires a browser from `provider` for `browserId` to drive; a raw CDP address
 * passes the network guard first, and an endpoint another browser holds is refused.
 */
export async function acquireSession(req, key, provider, wsUrl, browserId: string) {
  // http and https are here because Chrome prints its debugging port that way;
  // providers.ts resolves it through /json/version on the same host.
  const protocols = ['ws:', 'wss:', 'http:', 'https:'];
  if (provider === 'cdp' && wsUrl) await assertSafeTarget(wsUrl, { protocols, label: 'wsUrl' });
  const onCreated = (cleanup) => control().update(key, req.controlSession.id, { cleanup });
  const session = await acquireBrowser({ provider, wsUrl, env: keyConfig.envFor(key), onCreated });
  return reserve(session, key, browserId);
}

/**
 * The body for a start or attach that failed: the API's one error shape for a
 * refusal we raised, and for anything else (a socket that hung up, a vendor
 * that timed out) its message under provider_failed, since the message is
 * about the browser the caller named and is what they need to fix it.
 */
export const refusal = (err) =>
  err instanceof HttpError ? answerFor(err, {}).body : { error: err.message, code: 'provider_failed' };

/** Records the vendor's cleanup on the durable session, then checks provisioning may go on. */
export async function claimSession(key, browserId, session) {
  if (session.cleanup) await control().update(key, browserId, { cleanup: session.cleanup });
  await control().assertProvisioning(key, browserId);
}

/** Hands a vendor session back; a failure is logged, never thrown. */
export function releaseQuietly(session) {
  return session.release().catch((err) => console.error('[providers] cleanup:', err.message));
}

/** What a CDP browser brings besides its vendor session: the engine driving it, and optionally a persona and a release. */
type CdpExtra = Pick<BrowserSpec, 'engine' | 'persona' | 'release'>;

/** Registers a CDP browser this server dialled, and counts it for usage. */
export function addCdpBrowser(req, key, browserId, session, extra: CdpExtra) {
  const name = displayName(req, session);
  registry.add(browserId, { apiKey: key, name, clientType: 'cdp', provider: session.provider, ...extra });
  usage.browserConnected(key, browserId);
}
