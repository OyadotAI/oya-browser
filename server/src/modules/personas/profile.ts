/**
 * Profile generation: a full device drawn from a persona's seed, honouring its
 * prefs. Every seeded pick happens in a fixed order, so the same seed and prefs
 * always give the same device.
 */
import { createPRNG, pick } from './prng.ts';
import {
  GPU_DB,
  SCREEN_RESOLUTIONS,
  FONT_SETS,
  HARDWARE_CONCURRENCY,
  DEVICE_MEMORY,
  TIMEZONES,
  LOCALES,
  PLATFORMS,
  TIMEZONE_CHOICES,
  LOCALE_CHOICES,
} from './devices.ts';
import { AUDIO_NOISE_MAX, CANVAS_NOISE_MAX, COLOR_DEPTH, RECTS_NOISE_MAX, TASKBAR_PX } from './constants.ts';

/**
 * Personas created before prefs were validated (no `prefs.checked`) keep the
 * rule they were created under: a timezone or locale outside the platform's
 * seeded table falls back to the seeded pick. Widening it for them would
 * change their device under an existing cookie jar.
 */
function prefsFor(identity) {
  const p = identity?.prefs || {};
  const platform = PLATFORMS.includes(p.platform) ? p.platform : null;
  return { platform, timezone: p.timezone || null, locale: p.locale || null };
}

/**
 * A full device profile drawn from the persona's seed, honouring its prefs.
 *
 * @param {{ id: string, seed: number, proxy?: object|null }} identity
 *   An explicit id and numeric seed, so a persona's fingerprint is stable for
 *   its whole life and independent of what created it. Seeding from the API key
 *   made every browser on that key one device; seeding from an ephemeral
 *   browser id would give one account a new device on every restart. Neither is
 *   what a persona needs.
 */
export function generateProfile(identity) {
  const rng = createPRNG(identity.seed);
  const prefs = prefsFor(identity);
  const platform = pickPlatform(prefs, rng);
  const hardware = pickHardware(platform, rng);
  const place = pickPlace(identity, prefs, platform, rng);
  return buildProfile(identity, platform, hardware, place);
}

/**
 * The seeded picks still happen even when a preference overrides them, so
 * the rest of the stream — GPU, screen, noise seeds — is identical whether
 * or not a preference was given. A persona's fingerprint must depend on its
 * seed and its prefs only, never on the order they were applied.
 */
function pickPlatform(prefs, rng) {
  const platform = prefs.platform || pick(PLATFORMS, rng);
  if (prefs.platform) pick(PLATFORMS, rng);
  return platform;
}

/** GPU, screen, fonts, cores, memory and noise seeds, in stream order. */
function pickHardware(platform, rng) {
  return {
    gpu: pick(GPU_DB[platform] || GPU_DB.Win32, rng),
    screen: pick(SCREEN_RESOLUTIONS[platform] || SCREEN_RESOLUTIONS.Win32, rng),
    fonts: FONT_SETS[platform] || FONT_SETS.Win32,
    hardwareConcurrency: pick(HARDWARE_CONCURRENCY, rng),
    deviceMemory: pick(DEVICE_MEMORY, rng),
    noise: pickNoise(rng),
  };
}

/** Canvas, audio and client-rects noise seeds, in stream order. */
const pickNoise = (rng) => ({
  canvas: rng() * CANVAS_NOISE_MAX,
  audio: rng() * AUDIO_NOISE_MAX,
  rects: rng() * RECTS_NOISE_MAX,
});

/** Timezone and locale: the pref when the persona's rule allows it, else the seeded pick. */
function pickPlace(identity, prefs, platform, rng) {
  const checked = identity.prefs?.checked === true;
  const tzPick = pick(TIMEZONES[platform] || TIMEZONES.Win32, rng);
  const timezone = allowed(prefs.timezone, checked ? TIMEZONE_CHOICES : TIMEZONES[platform] || [], tzPick);
  const locPick = pick(LOCALES[platform] || LOCALES.Win32, rng);
  const locale = allowed(prefs.locale, checked ? LOCALE_CHOICES : LOCALES[platform] || [], locPick);
  // Chrome-shaped WebGL strings for personas made under the current rules;
  // older ones keep reporting what they always have (fingerprint.js, browser).
  return { timezone, locale, webglChrome: checked };
}

/** `value` when `list` has it, else `fallback`. */
const allowed = (value, list, fallback) => (list.includes(value) ? value : fallback);

/** The profile object browsers receive. */
function buildProfile(identity, platform, hardware, place) {
  return {
    id: identity.id,
    navigator: navigatorOf(platform, hardware, place.locale),
    screen: screenOf(hardware.screen),
    ...surfacesOf(hardware),
    ...place,
    proxy: identity.proxy || null,
  };
}

/** navigator.* values. */
function navigatorOf(platform, hardware, locale) {
  return {
    platform,
    hardwareConcurrency: hardware.hardwareConcurrency,
    deviceMemory: hardware.deviceMemory,
    maxTouchPoints: 0,
    languages: [locale, locale.split('-')[0]],
    vendor: 'Google Inc.',
  };
}

/** Colour and pixel depth, the same on every profile. */
const DEPTHS = { colorDepth: COLOR_DEPTH, pixelDepth: COLOR_DEPTH };

/** screen.* values, with the taskbar taken off the available height. */
function screenOf(screen) {
  return {
    width: screen.width,
    height: screen.height,
    availWidth: screen.width,
    availHeight: screen.height - TASKBAR_PX,
    ...DEPTHS,
    devicePixelRatio: screen.dpr,
  };
}

/** Canvas, WebGL, audio, rects and font surfaces. */
function surfacesOf(hardware) {
  return {
    canvas: { noiseSeed: hardware.noise.canvas },
    webgl: hardware.gpu,
    audio: { noiseSeed: hardware.noise.audio },
    rects: { noiseSeed: hardware.noise.rects },
    fonts: { available: hardware.fonts },
  };
}
