/**
 * Oya Cloud settings, read from OYA_CLOUD_* with the older DAYTONA_* names as
 * a fallback, and the naming conventions sandboxes are found by.
 */
import { createHash } from 'crypto';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { OWNER_TAG_CHARS, SHORT_ID_CHARS, MIN_SANDBOX_TTL_MINUTES, DEFAULT_SANDBOX_TTL_MINUTES } from '../constants.ts';

/** Sandboxes are named this plus the browser id, so a restart can still find one. */
export const PREFIX = 'oya-browser-';

/** Region used when none is configured. */
const DEFAULT_TARGET = 'us';

/**
 * The sandbox dials back to this server, so localhost cannot work from a
 * cloud VM — this is the one people hit and cannot diagnose.
 */
const TUNNEL_HINT =
  ' OYA_PUBLIC_WS_URL must be reachable from the sandbox, so a localhost' +
  ' server needs a tunnel (ngrok, cloudflared) rather than ws://localhost.';

/** Stable, non-reversible tag for the owning API key. Never label with the key itself. */
export const ownerTag = (apiKey) => createHash('sha256').update(apiKey).digest('hex').slice(0, OWNER_TAG_CHARS);

/** A cloud browser's name: the one given, or one made from its id. */
export const displayName = (name, browserId) => name || `Cloud browser ${browserId.slice(0, SHORT_ID_CHARS)}`;

/** Public name first, legacy name second. */
const setting = (env, name) => env[`OYA_CLOUD_${name}`] || env[`DAYTONA_${name}`] || '';

/** The cloud settings from env, or null unless API key, snapshot and public WebSocket URL are all set. */
export function settings(env = process.env) {
  const apiKey = setting(env, 'API_KEY');
  const snapshot = setting(env, 'SNAPSHOT');
  if (!apiKey || !snapshot || !env.OYA_PUBLIC_WS_URL) return null;
  return { apiKey, snapshot, wsUrl: env.OYA_PUBLIC_WS_URL, ...runtimeSettings(env) };
}

/** Where the runtime is and how long a sandbox may idle. */
function runtimeSettings(env) {
  return {
    apiUrl: setting(env, 'API_URL') || null,
    target: setting(env, 'TARGET') || DEFAULT_TARGET,
    // Abandoned sandboxes bill until something stops them. Tune per deployment.
    ttlMinutes: Math.max(
      MIN_SANDBOX_TTL_MINUTES,
      Number(setting(env, 'SANDBOX_TTL_MINUTES')) || DEFAULT_SANDBOX_TTL_MINUTES,
    ),
  };
}

/** True when this deployment can provision cloud browsers. */
export function isConfigured(env = process.env) {
  return settings(env) !== null;
}

/** Name what is actually missing. Listing all three when two are set sends
 *  people to re-check settings that were never the problem. */
export function missingSettings(env = process.env) {
  // Reported under the documented names, even when the legacy ones are in play.
  return [
    ['OYA_CLOUD_API_KEY', setting(env, 'API_KEY')],
    ['OYA_CLOUD_SNAPSHOT', setting(env, 'SNAPSHOT')],
    ['OYA_PUBLIC_WS_URL', env.OYA_PUBLIC_WS_URL],
  ]
    .filter(([, value]) => !value)
    .map(([name]) => name);
}

/** A 409 naming the missing settings, with a tunnel hint when the public WebSocket URL is the one missing. */
export function unconfigured(env = process.env) {
  const missing = missingSettings(env);
  const hint = missing.includes('OYA_PUBLIC_WS_URL') ? TUNNEL_HINT : '';
  const verb = missing.length > 1 ? 'are' : 'is';
  return new HttpError(Status.CONFLICT, `Cloud browsers need ${missing.join(', ')}, which ${verb} not set.${hint}`);
}
