/**
 * Rate limits and quotas, enforced.
 *
 * Rate limits are token buckets: O(1) per check, no timers, no sweep. A bucket
 * is created lazily and reclaimed once it has been full and idle, so 5k keys
 * do not leak 5k live objects.
 *
 * Quotas are point-in-time ceilings answered from live state (how many
 * browsers does this key have right now) or from usage.js (how much has it
 * spent this hour).
 *
 * Every limit is configurable; setting one to 0 or false disables it.
 */

import { metrics } from './metrics.ts';
import * as usage from './usage.ts';
import { Status } from './http-status.ts';
import {
  BEARER_PREFIX_LENGTH,
  DEFAULT_AGENT_SIGNUPS_PER_DAY,
  MINUTES_PER_DAY,
  DEFAULT_CHAT_BURST,
  DEFAULT_CHAT_PER_MIN,
  DEFAULT_CHAT_TOKENS_PER_HOUR,
  DEFAULT_COMMANDS_BURST,
  DEFAULT_COMMANDS_PER_MIN,
  DEFAULT_CONNECT_BURST,
  DEFAULT_CONNECT_PER_MIN,
  DEFAULT_MAX_BROWSERS,
  DEFAULT_PROVISION_BURST,
  DEFAULT_PROVISION_PER_MIN,
  DEFAULT_SANDBOXES_PER_HOUR,
  LIMIT_SWEEP_MS,
  MS_PER_MINUTE,
  SECONDS_PER_MINUTE,
} from './constants.ts';

/** A non-negative number from the environment; `false` means 0 (disabled), anything unparseable means the fallback. */
const num = (name, fallback) => {
  if (process.env[name] === 'false') return 0;
  const v = Number(process.env[name]);
  return Number.isFinite(v) && v >= 0 ? v : fallback;
};

/** perMinute 0 disables the limit. burst defaults to one minute's worth. */
export const LIMITS = {
  command: {
    perMinute: num('OYA_LIMIT_COMMANDS_PER_MIN', DEFAULT_COMMANDS_PER_MIN),
    burst: num('OYA_LIMIT_COMMANDS_BURST', DEFAULT_COMMANDS_BURST),
  },
  chat: {
    perMinute: num('OYA_LIMIT_CHAT_PER_MIN', DEFAULT_CHAT_PER_MIN),
    burst: num('OYA_LIMIT_CHAT_BURST', DEFAULT_CHAT_BURST),
  },
  provision: {
    perMinute: num('OYA_LIMIT_PROVISION_PER_MIN', DEFAULT_PROVISION_PER_MIN),
    burst: num('OYA_LIMIT_PROVISION_BURST', DEFAULT_PROVISION_BURST),
  },
  connect: {
    perMinute: num('OYA_LIMIT_CONNECT_PER_MIN', DEFAULT_CONNECT_PER_MIN),
    burst: num('OYA_LIMIT_CONNECT_BURST', DEFAULT_CONNECT_BURST),
  },
  // Keyed by caller address, not key: new agent keys per address, a day's worth at most.
  agentSignup: {
    perMinute: num('OYA_LIMIT_AGENT_SIGNUPS_PER_DAY', DEFAULT_AGENT_SIGNUPS_PER_DAY) / MINUTES_PER_DAY,
    burst: num('OYA_LIMIT_AGENT_SIGNUPS_PER_DAY', DEFAULT_AGENT_SIGNUPS_PER_DAY),
  },
};

/** Point-in-time ceilings per key; 0 disables one. */
export const QUOTAS = {
  browsers: num('OYA_QUOTA_MAX_BROWSERS', DEFAULT_MAX_BROWSERS),
  chatTokensPerHour: num('OYA_QUOTA_CHAT_TOKENS_HOUR', DEFAULT_CHAT_TOKENS_PER_HOUR),
  sandboxesPerHour: num('OYA_QUOTA_SANDBOXES_HOUR', DEFAULT_SANDBOXES_PER_HOUR),
};

const buckets = new Map(); // `${limit}:${key}` -> { tokens, updated }

/** Tops a bucket up for the time since it was last touched, capped at its burst. */
function refill(entry, cfg, now) {
  const elapsed = (now - entry.updated) / MS_PER_MINUTE;
  entry.tokens = Math.min(cfg.burst, entry.tokens + elapsed * cfg.perMinute);
  entry.updated = now;
}

/**
 * Consume one token.
 * @returns {{allowed: boolean, limit: number, remaining: number, retryAfter: number}}
 */
export function consume(name, key, cost = 1) {
  const cfg = LIMITS[name];
  if (!cfg || !cfg.perMinute || !key) return { allowed: true, limit: 0, remaining: Infinity, retryAfter: 0 };
  const entry = bucketFor(`${name}:${key}`, cfg);
  if (entry.tokens < cost) return refuse(name, key, cfg, cost - entry.tokens);
  entry.tokens -= cost;
  return { allowed: true, limit: cfg.perMinute, remaining: Math.floor(entry.tokens), retryAfter: 0 };
}

/** The bucket for this limit and key, created full on first use and refilled otherwise. */
function bucketFor(id, cfg) {
  const now = Date.now();
  let entry = buckets.get(id);
  if (!entry) {
    entry = { tokens: cfg.burst, updated: now };
    buckets.set(id, entry);
  } else refill(entry, cfg, now);
  return entry;
}

/** A refused consume: counted, and told how long until `shortfall` tokens have refilled. */
function refuse(name, key, cfg, shortfall) {
  const retryAfter = Math.ceil((shortfall / cfg.perMinute) * SECONDS_PER_MINUTE);
  metrics.rateLimited.inc({ limit: name });
  usage.record(key, 'rate_limited');
  return { allowed: false, limit: cfg.perMinute, remaining: 0, retryAfter: Math.max(1, retryAfter) };
}

/**
 * Check a ceiling. `current` is supplied by the caller so this module does not
 * reach into the registry and create a cycle.
 * @returns {{allowed: boolean, quota: number, current: number}}
 */
export function checkQuota(name, key, current) {
  const quota = QUOTAS[name];
  if (!quota) return { allowed: true, quota: 0, current };
  const allowed = current < quota;
  if (!allowed) {
    metrics.quotaExceeded.inc({ quota: name });
    usage.record(key, 'quota_denied');
  }
  return { allowed, quota, current };
}

/** Hourly spend ceilings, answered from this key's usage bucket. */
export function checkHourly(name, key) {
  const u = usage.current(key);
  if (name === 'chatTokensPerHour') {
    return checkQuota(name, key, (u.chat_input_tokens || 0) + (u.chat_output_tokens || 0));
  }
  if (name === 'sandboxesPerHour') return checkQuota(name, key, u.sandboxes_created || 0);
  return { allowed: true, quota: 0, current: 0 };
}

/** Express helper: applies a rate limit and answers with standard headers. */
export function enforce(name) {
  return (req, res, next) => {
    const result = consume(name, req.headers.authorization?.slice(BEARER_PREFIX_LENGTH) || '');
    res.set('RateLimit-Limit', String(result.limit));
    res.set('RateLimit-Remaining', String(Number.isFinite(result.remaining) ? result.remaining : 0));
    if (result.allowed) return next();
    tooManyRequests(res, name, result);
  };
}

/** Answer 429 with when to retry. */
function tooManyRequests(res, name, result) {
  res.set('Retry-After', String(result.retryAfter));
  res.status(Status.TOO_MANY_REQUESTS).json({
    error: `Rate limit exceeded for ${name}`,
    limit: result.limit,
    retryAfter: result.retryAfter,
  });
}

/** What this key is currently allowed, without consuming anything. */
export function status(key) {
  const now = Date.now();
  const out = {};
  for (const [name, cfg] of Object.entries(LIMITS)) out[name] = limitStatus(name, key, cfg, now);
  return { limits: out, quotas: QUOTAS };
}

/** One limit's standing for a key: its rate, burst and tokens left, or disabled. */
function limitStatus(name, key, cfg, now) {
  if (!cfg.perMinute) return { limit: 0, remaining: Infinity, disabled: true };
  const entry = buckets.get(`${name}:${key}`);
  if (entry) refill(entry, cfg, now);
  return { limit: cfg.perMinute, burst: cfg.burst, remaining: Math.floor(entry ? entry.tokens : cfg.burst) };
}

/**
 * Reclaim buckets that are full (nothing owed) and idle. Full means the key
 * has spent nothing recently, so dropping it loses no state.
 */
function sweepBuckets() {
  const now = Date.now();
  for (const [id, entry] of buckets) sweepBucket(id, entry, now);
}

/** Drop one bucket if its limit is gone, or if it is full and idle past the sweep interval. */
function sweepBucket(id, entry, now) {
  const cfg = LIMITS[id.slice(0, id.indexOf(':'))];
  if (!cfg) {
    buckets.delete(id);
    return;
  }
  refill(entry, cfg, now);
  if (entry.tokens >= cfg.burst && now - entry.updated > LIMIT_SWEEP_MS) buckets.delete(id);
}

const sweep = setInterval(sweepBuckets, LIMIT_SWEEP_MS);
sweep.unref?.();

/** Test hook. */
export function reset() {
  buckets.clear();
}
