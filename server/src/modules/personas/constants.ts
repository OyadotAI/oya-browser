/**
 * Every number the personas module runs on, by name: concurrency caps, limits
 * on what callers may store, timings, and the constants that pin a persona's
 * device to its seed. Values an operator may tune come from the environment.
 */

/** Browsers a named persona may run at once unless OYA_PERSONA_MAX_CONCURRENT says otherwise. */
const DEFAULT_NAMED_PERSONA_CAP = 2;

/**
 * A named persona is one device, so it gets a low cap, a person has a phone
 * and a laptop, not a thousand.
 */
export const DEFAULT_MAX_CONCURRENT = Math.max(
  1,
  Number(process.env.OYA_PERSONA_MAX_CONCURRENT) || DEFAULT_NAMED_PERSONA_CAP,
);

/**
 * The default persona is the migration path for keys that already run many
 * browsers, so it is uncapped unless the operator says otherwise. Those fleets
 * are exposed by concurrency rather than fingerprint diversity, which is what
 * the activeBrowsers metric surfaces; capping them here would break a working
 * setup on upgrade.
 */
export const DEFAULT_PERSONA_MAX_CONCURRENT = Number(process.env.OYA_DEFAULT_PERSONA_MAX_CONCURRENT) || Infinity;

/** Longest persona display name kept. */
export const MAX_NAME_CHARS = 100;
/** Longest device choice (platform, timezone, locale) kept. */
export const MAX_PREF_CHARS = 64;
/** How often changed personas are written out. */
export const AUTOSAVE_MS = 10_000;
/** Indentation of the personas JSON file. */
export const JSON_INDENT = 2;

// ── Login state (cookies.ts) ──

/** Changes are batched into one save this long after the first. */
export const LOGIN_SAVE_DELAY_MS = 500;
/** Format of the sealed login-state file. */
export const LOGIN_FILE_VERSION = 2;
/** Cookie expiry is in seconds; Date.now() is in milliseconds. */
export const MS_PER_SECOND = 1000;
/** Hosts one cookie lookup may ask about. */
export const MAX_COOKIE_HOSTS = 20;
/** Origins accepted per localStorage merge. */
export const MAX_STORAGE_ORIGINS = 100;
/** Longest localStorage key kept. */
export const MAX_STORAGE_KEY_CHARS = 8192;
/** An origin whose storage serialises larger than this (2 MiB) is skipped. */
export const MAX_ORIGIN_STORAGE_CHARS = 2_097_152;

// ── Site credentials ──

/** Labels in a registrable domain (`example.com`), the parent a credential lookup falls back to. */
export const REGISTRABLE_LABELS = 2;

// ── Fingerprint seeds ──

/** Hex characters of the key hash that name a key's default persona. */
export const DEFAULT_ID_HEX_CHARS = 12;
/** Hex characters of the hash that names a persona mirroring one real profile. */
export const MIRROR_ID_HEX_CHARS = 16;
/** Random bytes in a new persona's id. */
export const PERSONA_ID_BYTES = 8;
/** LCG multiplier (Numerical Recipes); must match the browser side. */
export const LCG_MULTIPLIER = 1664525;
/** LCG increment (Numerical Recipes); must match the browser side. */
export const LCG_INCREMENT = 1013904223;
/** Largest unsigned 32-bit value: the LCG's mask and divisor. */
export const UINT32_MAX = 0xffffffff;
/** The string hash is `hash * 31 + char`, done as a shift by 5 minus hash. */
export const HASH_SHIFT = 5;

// ── Fingerprint surfaces ──

/** Upper bound of the canvas noise seed. */
export const CANVAS_NOISE_MAX = 0.01;
/** Upper bound of the audio noise seed. */
export const AUDIO_NOISE_MAX = 0.01;
/** Upper bound of the client-rects noise seed. */
export const RECTS_NOISE_MAX = 0.001;
/** Screen height lost to the taskbar or dock. */
export const TASKBAR_PX = 40;
/** Colour and pixel depth every profile reports. */
export const COLOR_DEPTH = 24;

/** Logical core counts common on consumer machines. */
export const CoreCount = { FOUR: 4, SIX: 6, EIGHT: 8, TWELVE: 12, SIXTEEN: 16 } as const;
/** navigator.deviceMemory values, in GB. */
export const MemoryGb = { FOUR: 4, EIGHT: 8, SIXTEEN: 16 } as const;
