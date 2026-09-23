/**
 * Which API keys exist: env admin keys, the fleet token, and the digests of
 * stored keys, cached in memory from Supabase on startup.
 */

import { createHash, randomBytes } from 'crypto';
import { db as supabase } from '../../platform/db.ts';
import { KEY_BYTES, KEY_PREFIX_CHARS } from './constants.ts';

// ── Env-configured admin keys ──

/** Keys listed in API_KEYS. */
const envKeys = new Set(
  (process.env.API_KEYS || '')
    .split(',')
    .map((k) => k.trim())
    .filter(Boolean),
);

// ── Fleet token ──

/** The configured fleet token, or null. */
const fleetToken = (process.env.FLEET_TOKEN || '').trim() || null;

// ── Key digests ──

/**
 * The stored form of a key. Nothing here ever persists the key itself: a
 * backup, a replica, a support export or an over-broad grant on api_keys then
 * yields administrator credentials for every tenant's browsers, cookie jars and
 * personas. The rest of the control plane already worked this way, audit.js,
 * key-config.js, personas and sandbox labels all record a digest.
 */
export const keyDigest = (key) => createHash('sha256').update(String(key)).digest('hex');

/** What a person sees in a key list. The key itself is shown once, at creation. */
export const keyPrefix = (key) => String(key).slice(0, KEY_PREFIX_CHARS);

/** A fresh random API key. */
export const generateKey = () => randomBytes(KEY_BYTES).toString('base64url');

/** Keys this process still holds in the clear: the env ones and the fleet token. */
export const knownKeys = () => [...new Set([...envKeys, ...(fleetToken ? [fleetToken] : [])])];

/** Whether a key is listed in API_KEYS. */
export const isEnvKey = (key) => envKeys.has(key);

/**
 * Keys listed in API_KEYS exist for self-hosting without a database. They are
 * ordinary keys: env grants existence, never authority. Host-level operations
 * use OYA_OPERATOR_TOKEN, which is not an API key at all.
 */

/** Whether a key is the configured fleet token. */
export function isFleetToken(key) {
  return fleetToken !== null && key === fleetToken;
}

// ── In-memory cache of key digests (loaded from Supabase on startup) ──

/** sha256 hex digests, never keys. */
export const keyCache = new Set();
/** Whether the cache has been filled from Supabase. */
let loaded = false;

/** Loads every stored key digest into the cache once; a failure is logged and retried on the next call. */
async function loadKeys() {
  if (!supabase || loaded) return;
  try {
    await fillCache();
  } catch (e) {
    console.error('[auth] Failed to load keys:', e.message);
  }
}

/** Reads every stored digest into the cache and marks it loaded. */
async function fillCache() {
  const { data, error } = await supabase.from('api_keys').select('key_hash');
  if (error) throw error;
  for (const row of data) keyCache.add(row.key_hash);
  loaded = true;
  console.log(`[auth] Loaded ${data.length} API key digests from Supabase`);
}

/** Readiness is shared with server startup; an empty cache is not an invalid key. */
export const authReady = loadKeys().then(() => !supabase || loaded);

/** Whether a key is an env key, a known stored key or the fleet token. */
export function validateApiKey(key) {
  if (!key) return false;
  return envKeys.has(key) || keyCache.has(keyDigest(key)) || isFleetToken(key);
}

// ── Agent keys no person has claimed ──

/**
 * Digests of agent keys nobody has claimed, as this process last saw them.
 * Every API request re-reads its key's row (middleware.ts isStoredKey), so a
 * claim on another instance is picked up on the key's next request here.
 */
const unclaimedAgents = new Set<string>();

/** Records whether a key's row is an agent's own, unclaimed key. */
export function noteAgentKey(digest: string, unclaimed: boolean) {
  if (unclaimed) unclaimedAgents.add(digest);
  else unclaimedAgents.delete(digest);
}

/** Whether `key` is an agent's key no person has claimed: it brings its own LLM and cloud browsers need a claim. */
export const agentKeyUnclaimed = (key) => Boolean(key) && unclaimedAgents.has(keyDigest(key));
