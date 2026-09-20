/**
 * Server-side fingerprint generation — single source of truth.
 * Generates deterministic fingerprint profiles from API keys and caches them.
 * Sent to browsers on auth_ok so every instance gets the exact same profile.
 */

import { createHash, randomBytes } from 'crypto';
import { PLATFORMS, TIMEZONE_CHOICES, LOCALE_CHOICES } from './devices.ts';
import { seedFromString } from './prng.ts';
import { generateProfile } from './profile.ts';
import { DEFAULT_ID_HEX_CHARS, PERSONA_ID_BYTES } from './constants.ts';

export { PLATFORMS };

/** The choices a persona may make, per platform; served as the persona options list. */
export const PREF_OPTIONS = {
  platforms: PLATFORMS,
  timezones: Object.fromEntries(PLATFORMS.map((p) => [p, TIMEZONE_CHOICES])),
  locales: Object.fromEntries(PLATFORMS.map((p) => [p, LOCALE_CHOICES])),
};

/**
 * Why these prefs cannot be honoured, or null. Checked at creation, so a
 * persona never silently gets a device other than the one it asked for.
 */
export function prefsError(prefs) {
  if (!prefs || typeof prefs !== 'object') return null;
  return (
    notOffered('platform', prefs.platform, PLATFORMS) ||
    notOffered('timezone', prefs.timezone, TIMEZONE_CHOICES) ||
    notOffered('locale', prefs.locale, LOCALE_CHOICES)
  );
}

/** The refusal for a chosen `value` that `list` does not offer, or null. */
const notOffered = (key, value, list) =>
  typeof value === 'string' && value && value !== 'auto' && !list.includes(value)
    ? `${key} "${value}" is not offered for personas; GET /api/personas/options lists the choices`
    : null;

// ── Cache: one profile per persona, generated once ──

/** Persona id -> its generated profile. */
const cache = new Map();

/** The seed and id a key's default persona uses. */
export function defaultPersonaSeed(apiKey) {
  return {
    id: 'apikey-' + createHash('sha256').update(apiKey).digest('hex').slice(0, DEFAULT_ID_HEX_CHARS),
    seed: seedFromString(apiKey),
  };
}

/** A fresh persona's seed, independent of any key. */
export function newPersonaSeed() {
  const id = 'p-' + randomBytes(PERSONA_ID_BYTES).toString('hex');
  return { id, seed: seedFromString(id) };
}

/** Unmemoised: for previews, which must not accumulate in the cache. */
export function previewProfile(identity) {
  return generateProfile(identity);
}

/**
 * Stable fingerprint for a persona. Same persona in, same fingerprint out,
 * for the life of the persona — that stability is what keeps the fingerprint
 * coherent with the cookies it is paired with.
 */
export function getFingerprintForPersona(identity) {
  if (!identity?.id) return null;
  let profile = cache.get(identity.id);
  if (!profile) {
    profile = generateProfile(identity);
    cache.set(identity.id, profile);
    console.log(`[fingerprint] Generated ${profile.id} (${profile.navigator.platform}, ${profile.timezone})`);
  }
  return profile;
}

/**
 * Back-compat for callers that still think in API keys: resolves to that key's
 * default persona, preserving the exact fingerprint it had before personas
 * existed — same id, same seed, same output.
 */
export function getFingerprintForKey(apiKey) {
  if (!apiKey) return null;
  return getFingerprintForPersona(defaultPersonaSeed(apiKey));
}
