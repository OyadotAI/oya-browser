/**
 * Validation of a provider config, from the API, OYA_PROVIDERS or a saved
 * routing setting. A bad name, type, CDP URL or limit is refused with a 400.
 */
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { DEFAULT_MAX_CONCURRENT, DEFAULT_PRIORITY } from './constants.ts';

/** Numeric settings: [field, default, minimum]. */
const LIMITS: [string, number, number][] = [
  ['maxConcurrent', DEFAULT_MAX_CONCURRENT, 1],
  ['priority', DEFAULT_PRIORITY, 0],
  ['weight', 1, 1],
];

/** Refuses the config with a 400. */
function fail(message: string): never {
  throw new HttpError(Status.BAD_REQUEST, message);
}

/** Normalize a provider config and reject a bad name, type, CDP URL or limit with a 400. */
export function validateProviderConfig(cfg) {
  const name = checkName(cfg);
  const type = checkType(cfg);
  const wsUrl = type === 'cdp' ? cdpUrl(cfg) : null;
  const result = { name, owner: cfg.owner ?? null, type, wsUrl, enabled: cfg.enabled !== false };
  for (const [field, fallback, min] of LIMITS) result[field] = checkLimit(cfg, field, fallback, min);
  return result;
}

/** The trimmed name: 1–80 letters, numbers, spaces, dots, underscores or hyphens. */
function checkName(cfg) {
  if (!cfg || typeof cfg.name !== 'string' || !/^[a-zA-Z0-9][a-zA-Z0-9 ._-]{0,79}$/.test(cfg.name.trim())) {
    fail('Use a provider name of 1–80 letters, numbers, spaces, dots, underscores, or hyphens.');
  }
  return cfg.name.trim();
}

/** The provider type, 'cdp' when none is given. */
function checkType(cfg) {
  const type = cfg.type || 'cdp';
  if (typeof type !== 'string' || !/^[a-z][a-z0-9_-]*$/.test(type)) fail('Invalid provider type');
  return type;
}

/** The normalized ws:// or wss:// URL a 'cdp' provider requires. */
function cdpUrl(cfg) {
  try {
    const url = new URL(cfg.wsUrl);
    if (!['ws:', 'wss:'].includes(url.protocol)) throw new Error();
    return url.href;
  } catch {
    fail('A ws:// or wss:// CDP WebSocket URL is required.');
  }
}

/** One numeric setting: the default when absent, else an integer of at least `min`. */
function checkLimit(cfg, field, fallback, min) {
  const value = cfg[field] === undefined ? fallback : Number(cfg[field]);
  if (cfg[field] === null || cfg[field] === '' || !Number.isSafeInteger(value) || value < min)
    fail(`${field} must be an integer of at least ${min}.`);
  return value;
}
