/**
 * User agent and client hints, derived from a persona.
 *
 * Two things go wrong here and both are cheap to detect:
 *
 *   1. `HeadlessChrome` in the UA — the oldest check there is — and an OS token
 *      that contradicts the fingerprint's `navigator.platform`. A Win32 profile
 *      behind a `Macintosh` UA is a stronger signal than either alone.
 *   2. Sending `Emulation.setUserAgentOverride` with a `platform` but no
 *      `userAgentMetadata`. Chrome then blanks client hints entirely:
 *      `navigator.userAgentData.brands` becomes `[]` and platform `''`. No real
 *      browser does that, so the mitigation plants the flag it was hiding.
 */

/** Chrome's UA has been frozen for years; only the major version moves. */
const UA_TEMPLATES = {
  Win32:
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/%V%.0.0.0 Safari/537.36',
  MacIntel:
    'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/%V%.0.0.0 Safari/537.36',
  'Linux x86_64':
    'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/%V%.0.0.0 Safari/537.36',
};

/** Client-hint platform fields for each navigator.platform. */
const HINTS = {
  Win32: { platform: 'Windows', platformVersion: '15.0.0', architecture: 'x86', bitness: '64' },
  MacIntel: { platform: 'macOS', platformVersion: '14.6.1', architecture: 'x86', bitness: '64' },
  'Linux x86_64': { platform: 'Linux', platformVersion: '', architecture: 'x86', bitness: '64' },
};

/** The profile's platform as a template key; unknown Linux flavours map to x86_64, anything else to Windows. */
const platformOf = (profile) => {
  const p = profile?.navigator?.platform || '';
  if (UA_TEMPLATES[p]) return p;
  if (p.startsWith('Linux')) return 'Linux x86_64';
  return 'Win32';
};

/** Chrome's major version from a UA string, or '' when there is none. */
export const majorFrom = (ua) => (String(ua).match(/Chrome\/(\d+)/) || [])[1] || '';

/** The UA string this persona should present. */
export function userAgentFor(profile, browserUa) {
  const major = majorFrom(browserUa);
  const template = UA_TEMPLATES[platformOf(profile)];
  if (!major) return String(browserUa || '').replace(/HeadlessChrome/g, 'Chrome');
  return template.replace('%V%', major);
}

/**
 * Client hints to send alongside the override.
 *
 * `brands` should be the browser's own list, read before overriding: the GREASE
 * entry ("Not?A_Brand" and friends) changes between releases and cannot be
 * guessed. A constructed list is the fallback, and still beats an empty one.
 */
export function metadataFor(profile, browserUa, brands) {
  const major = majorFrom(browserUa) || '131';
  const list = Array.isArray(brands) && brands.length ? brands : defaultBrands(major);
  return { ...versionsOf(list, major), ...hintsFor(profile), ...DESKTOP };
}

/** The brand list, the same with full versions, and Chrome's full version. */
const versionsOf = (list, major) => ({
  brands: list,
  fullVersionList: list.map(fullVersionOf),
  fullVersion: `${major}.0.0.0`,
});

/** The platform's client hints, with the architecture an Apple GPU implies. */
function hintsFor(profile) {
  const key = platformOf(profile);
  const hints = { ...HINTS[key] };
  // An Apple GPU means Apple Silicon, which reports arm while keeping
  // navigator.platform at MacIntel — that pairing is what a real M-series Mac
  // looks like, and claiming x86 next to an M-series renderer is a mismatch.
  if (key === 'MacIntel' && /Apple M/i.test(profile?.webgl?.renderer || '')) {
    hints.architecture = 'arm';
  }
  return hints;
}

/** Chrome's own brand list for `major`, used when the browser's was not read. */
const defaultBrands = (major) => [
  { brand: 'Chromium', version: major },
  { brand: 'Not?A_Brand', version: '24' },
  { brand: 'Google Chrome', version: major },
];

/** A brand with a full version: a bare major becomes `major.0.0.0`. */
const fullVersionOf = (b) => ({
  brand: b.brand,
  version: /^\d+$/.test(String(b.version)) ? `${b.version}.0.0.0` : String(b.version),
});

/** A desktop browser: no device model, not mobile, not WOW64. */
const DESKTOP = { model: '', mobile: false, wow64: false };
