/**
 * Where a CDP browser comes from.
 *
 * Two kinds:
 *   'cdp'  — you already have a CDP WebSocket URL (plain Chrome, your own
 *            infra, a tunnel). Always correct, no vendor knowledge needed.
 *   hosted — ask a vendor's REST API for a session, then drive the CDP URL it
 *            returns.
 *
 * Hosted providers are described by config rather than code, so a vendor
 * changing its API can be handled with OYA_BROWSER_PROVIDERS overrides.
 * deleteUrl also accepts a URL template containing {id}; deleteMethod and
 * deleteBody describe the vendor's release operation.
 */
import { openText } from '../platform/secrets.ts';
import { Status } from '../platform/http-status.ts';
import { catalog, keyFor, firstPath } from './providers/catalog.ts';
import {
  fail,
  parseUrl,
  isWebSocket,
  isGone,
  createSession,
  cdpUrl,
  releaser,
  cleanupRecord,
} from './providers/vendor-session.ts';
import { PROVIDER_RELEASE_TIMEOUT_MS } from './constants.ts';

/** One provider as the API lists it. */
export interface ProviderInfo {
  /** 'cdp' or the vendor name. */
  name: string;
  /** 'direct' for cdp, 'hosted' for vendors. */
  kind: string;
  /** Has an API key and a create URL. */
  configured: boolean;
  /** A short hint about how the provider is used. */
  note?: string;
  /** Where the vendor's API key is read from. */
  envVar?: string;
}

/** Providers this deployment can actually use right now. */
export function available(env = process.env) {
  const out: ProviderInfo[] = [
    // Always usable: it needs no vendor and no key.
    { name: 'cdp', kind: 'direct', configured: true, note: 'Bring your own CDP WebSocket URL' },
  ];
  for (const [name, cfg] of Object.entries(catalog(env)) as [string, any][]) out.push(hostedInfo(name, cfg, env));
  return out;
}

/** A hosted vendor's listing: configured once it has both a key and a create URL. */
function hostedInfo(name, cfg, env): ProviderInfo {
  const envVar = `${name.toUpperCase()}_API_KEY`;
  return { name, kind: 'hosted', configured: !!keyFor(name, env) && !!cfg.createUrl, envVar };
}

/**
 * Acquire a CDP endpoint.
 * @returns {{ wsUrl: string, provider: string, sessionId: string|null, release: () => Promise<void> }}
 */
export async function acquire({ provider = 'cdp', wsUrl, env = process.env, onCreated }: any = {}) {
  if (provider === 'cdp') return direct(wsUrl, provider);
  const { cfg, key } = vendorFor(provider, env);
  const payload = await createSession(provider, cfg, key, env);
  const sessionId = firstPath(payload, cfg.idPath || []);
  const release = releaser(provider, cfg, key, sessionId);
  const cleanup = cleanupRecord(cfg, key, sessionId);
  return connectOrRelease({ provider, cfg, key, payload, sessionId, release, cleanup, onCreated });
}

/** A CDP URL the caller already has: checked, and nothing to release. */
function direct(wsUrl, provider) {
  if (!wsUrl) fail('wsUrl is required for the cdp provider');
  const parsed = parseUrl(wsUrl);
  if (!parsed) fail('wsUrl is not a valid URL');
  if (!isWebSocket(parsed)) fail('wsUrl must be ws:// or wss://');
  return { wsUrl, provider, sessionId: null, release: async () => {} };
}

/** The vendor's config and API key; unknown or unconfigured vendors are refused. */
function vendorFor(provider, env) {
  const cfg = catalog(env)[provider];
  if (!cfg?.createUrl) fail(`Unknown browser provider: ${provider}`, Status.BAD_REQUEST);
  const key = keyFor(provider, env);
  if (!key) fail(`${provider} is not configured — set ${provider.toUpperCase()}_API_KEY`, Status.CONFLICT);
  return { cfg, key };
}

/** Records the cleanup and reads the CDP URL; if either fails the session is released, not leaked. */
async function connectOrRelease({ provider, cfg, key, payload, sessionId, release, cleanup, onCreated }) {
  try {
    if (cleanup && onCreated) await onCreated(cleanup);
    return { wsUrl: cdpUrl(provider, cfg, key, payload), provider, sessionId, release, cleanup };
  } catch (err) {
    await noteCleanupFailure(err, release);
    throw err;
  }
}

/** Releases after a failure; a release that also fails is appended to the original error. */
async function noteCleanupFailure(err, release) {
  try {
    await release();
  } catch (cleanup) {
    err.message += ` Cleanup also failed: ${cleanup.message}`;
  }
}

/** Release a vendor session from the sealed cleanup record acquire() handed to onCreated, used by the control worker. 404/410 count as released. */
export async function releasePersisted(cleanup) {
  const { url, ...options } = openText('provider-cleanup', cleanup.sealed);
  const signal = AbortSignal.timeout(PROVIDER_RELEASE_TIMEOUT_MS);
  const response = await fetch(url, { ...options, redirect: 'error', signal });
  if (!response.ok && !isGone(response.status)) throw new Error(`Provider release failed (${response.status})`);
}
