/**
 * Oya Cloud settings: which runtime cloud browsers run on (OYA_CLOUD_RUNTIME),
 * that runtime's own settings, and the ones every runtime shares, the public
 * WebSocket URL a sandbox dials back to and how long it may idle.
 */
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { setting } from './names.ts';
import { WORKERS, workerFor } from './worker.ts';
import { MIN_SANDBOX_TTL_MINUTES, DEFAULT_SANDBOX_TTL_MINUTES } from '../constants.ts';

export { PREFIX, ownerTag, displayName } from './names.ts';

/**
 * The sandbox dials back to this server, so localhost cannot work from a
 * cloud VM, this is the one people hit and cannot diagnose.
 */
const TUNNEL_HINT =
  ' OYA_PUBLIC_WS_URL must be reachable from the sandbox, so a localhost' +
  ' server needs a tunnel (ngrok, cloudflared) rather than ws://localhost.';

/** The settings for the selected runtime, or null unless it and the public WebSocket URL are all set. */
export function settings(env = process.env) {
  const worker = workerFor(env);
  const own = worker?.settings(env);
  if (!own || !env.OYA_PUBLIC_WS_URL) return null;
  return { runtime: worker.id, ...own, wsUrl: env.OYA_PUBLIC_WS_URL, ttlMinutes: ttlMinutes(env) };
}

/** How long a sandbox may idle; abandoned sandboxes bill until something stops them. */
const ttlMinutes = (env) =>
  Math.max(MIN_SANDBOX_TTL_MINUTES, Number(setting(env, 'SANDBOX_TTL_MINUTES')) || DEFAULT_SANDBOX_TTL_MINUTES);

/** True when this deployment can provision cloud browsers. */
export function isConfigured(env = process.env) {
  return settings(env) !== null;
}

/** Name what is actually missing. Listing all of them when two are set sends
 *  people to re-check settings that were never the problem. */
export function missingSettings(env = process.env) {
  const worker = workerFor(env);
  const runtime = worker ? worker.missing(env) : [`OYA_CLOUD_RUNTIME (one of ${Object.keys(WORKERS).join(', ')})`];
  return [...runtime, ...(env.OYA_PUBLIC_WS_URL ? [] : ['OYA_PUBLIC_WS_URL'])];
}

/** A 409 naming the missing settings, with a tunnel hint when the public WebSocket URL is the one missing. */
export function unconfigured(env = process.env) {
  const missing = missingSettings(env);
  const hint = missing.includes('OYA_PUBLIC_WS_URL') ? TUNNEL_HINT : '';
  const verb = missing.length > 1 ? 'are' : 'is';
  return new HttpError(Status.CONFLICT, `Cloud browsers need ${missing.join(', ')}, which ${verb} not set.${hint}`);
}
