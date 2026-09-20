/**
 * The persona model.
 *
 * A persona is one identity: a fingerprint, a cookie jar and a proxy, bound
 * together and stable for its life. That binding is the whole point — one
 * account arriving from many device fingerprints is a bot-farm signal, and so
 * is many accounts arriving from one. Rotation means picking a different
 * persona, never giving a persona a new fingerprint.
 *
 * An API key groups personas; it is not itself one. Every key has a default
 * persona whose seed reproduces the fingerprint that key had before personas
 * existed, so nothing changes for a customer running a single account.
 *
 * Concurrency is capped per persona because one device cannot be in a thousand
 * places at once — at fleet scale that, not fingerprint diversity, is the
 * exposure.
 */
import { DEFAULT_MAX_CONCURRENT, MAX_PREF_CHARS } from './constants.ts';

/** Device choices made at creation. `checked` marks prefs validated under the current rule. */
export type PersonaPrefs = {
  /** Requested OS platform. */
  platform?: string;
  /** Requested IANA timezone. */
  timezone?: string;
  /** Requested browser locale. */
  locale?: string;
  /** Set when the prefs were validated under the current rule. */
  checked?: boolean;
};

/** The persona's own proxy, as the caller gave it; fields beyond these pass through untouched. */
export type PersonaProxy = {
  /** Proxy hostname. */
  host?: string;
  /** Proxy port. */
  port?: number;
  /** Exit country, leading with its two-letter code; checked against the fingerprint's timezone. */
  geo?: string;
  [field: string]: unknown;
};

/** One identity: a fixed device, a cookie jar and a proxy, owned by one API key. */
export interface Persona {
  /** Stable persona id. */
  id: string;
  /** Fingerprint of the owning API key: the tenancy boundary. */
  owner: string;
  /** Display name. */
  name: string;
  /** With prefs, this is the device. Neither ever changes. */
  seed: number;
  /** Device choices made at creation, or null to let the seed decide. */
  prefs: PersonaPrefs | null;
  /** The persona's own proxy, or null to use an assigned or residential one. */
  proxy: PersonaProxy | null;
  /** How many browsers may run as this persona at once; Infinity for no cap. */
  maxConcurrent: number;
  /** Whether this is the key's default persona. */
  isDefault: boolean;
  /** ISO time the persona was created. */
  createdAt: string;
  /** ISO time a browser last started as it, or null if never. */
  lastUsedAt: string | null;
}

export { DEFAULT_MAX_CONCURRENT, DEFAULT_PERSONA_MAX_CONCURRENT } from './constants.ts';

/** A persona from any source (a request, a file row, a database row), normalised. Storage writes Infinity as null. */
export const shape = (p: any): Persona => ({
  id: p.id,
  owner: p.owner,
  name: p.name,
  ...deviceOf(p),
  ...usageOf(p),
});

/** The device half of a persona: seed, prefs and proxy. */
const deviceOf = (p: any) => ({
  seed: p.seed,
  // Device choices made at creation. Immutable with the seed: together they
  // are the fingerprint, and the fingerprint is what must not change.
  prefs: p.prefs && typeof p.prefs === 'object' ? { ...p.prefs } : null,
  proxy: p.proxy || null,
});

/** How a persona is used: its cap, whether it is the default, and when it was made and last run. */
const usageOf = (p: any) => ({
  maxConcurrent: p.maxConcurrent === null ? Infinity : (p.maxConcurrent ?? DEFAULT_MAX_CONCURRENT),
  isDefault: !!p.isDefault,
  createdAt: p.createdAt,
  lastUsedAt: p.lastUsedAt || null,
});

/**
 * Prefs validated at creation carry `checked`, inside prefs so it persists
 * wherever prefs do. Unmarked (older) prefs keep the rule they were created
 * under, or their device would move under its cookie jar (fingerprint.ts).
 */
export const markChecked = (prefs: PersonaPrefs | null, checked: boolean) =>
  checked ? { ...(prefs || {}), checked: true } : prefs;

/** The device choices a caller made, without the internal marker. */
export const publicPrefs = (prefs: PersonaPrefs | null) => {
  if (!prefs) return null;
  const { checked, ...choices } = prefs;
  return Object.keys(choices).length ? choices : null;
};

/** Only the three device choices, only as strings. Anything else is dropped. */
export function cleanPrefs(prefs: unknown): PersonaPrefs | null {
  if (!prefs || typeof prefs !== 'object') return null;
  const out: PersonaPrefs = {};
  for (const k of ['platform', 'timezone', 'locale'] as const) {
    const value = (prefs as Record<string, unknown>)[k];
    if (typeof value === 'string' && value && value !== 'auto') out[k] = value.slice(0, MAX_PREF_CHARS);
  }
  return Object.keys(out).length ? out : null;
}

/** The public view of a fingerprint: enough to recognise the device, no seeds. */
export function describeProfile(fp: any) {
  return {
    platform: fp.navigator.platform,
    timezone: fp.timezone,
    locale: fp.locale,
    screen: `${fp.screen.width}x${fp.screen.height}`,
    webgl: fp.webgl.unmaskedRenderer,
    ...hardwareOf(fp),
  };
}

/** The hardware a fingerprint claims, and its canvas noise seed. */
const hardwareOf = (fp: any) => ({
  hardwareConcurrency: fp.navigator.hardwareConcurrency,
  deviceMemory: fp.navigator.deviceMemory,
  canvasSeed: fp.canvas.noiseSeed,
});
