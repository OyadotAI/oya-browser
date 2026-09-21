/**
 * The browser a persona says it is, from one place: the User-Agent string, the
 * page-side override (navigator.userAgent and navigator.userAgentData) and the
 * client-hint headers. They used to be built apart, and only the headers claimed
 * Google Chrome: a page's JavaScript still saw Electron's own brands and the
 * host's OS. Google's sign-in compares the two and refused the browser as "may
 * not be secure". Built together, they cannot disagree.
 *
 * The version is always the engine's own. Claiming a newer Chrome than the one
 * running (a mirrored profile's, say) is caught by testing for features that
 * version has, so it is never claimed.
 */
const { FALLBACK_CHROME_VERSION } = require('./constants.cjs');

/** What each navigator.platform looks like: `os` in the UA string, `hints` in navigator.userAgentData. */
const PLATFORMS = {
  Win32: { os: 'Windows NT 10.0; Win64; x64', hints: { platform: 'Windows', platformVersion: '15.0.0' } },
  MacIntel: { os: 'Macintosh; Intel Mac OS X 10_15_7', hints: { platform: 'macOS', platformVersion: '14.6.1' } },
  'Linux x86_64': { os: 'X11; Linux x86_64', hints: { platform: 'Linux', platformVersion: '' } },
};

/** The navigator.platform of this machine, for a browser with no persona yet. */
const HOST_PLATFORMS = { darwin: 'MacIntel', win32: 'Win32' };

/** Chromium's GREASE alphabet, versions and brand orders (components/embedder_support/user_agent_utils.cc). */
const GREASE_CHARS = [' ', '(', ':', '-', '.', '/', ')', ';', '=', '?', '_'];
/** The versions a GREASE brand takes. */
const GREASE_VERSIONS = ['8', '99', '24'];
/** The slots the GREASE, Chromium and Google Chrome brands take, in that order, by `major % 6`. */
const BRAND_ORDERS = ['012', '021', '102', '120', '201', '210'];

/** The engine's Chrome version: Electron's own, else the one in `defaultUA`, else a known one. */
function engineVersion(defaultUA = '') {
  return process.versions.chrome || defaultUA.match(/Chrome\/([\d.]+)/)?.[1] || FALLBACK_CHROME_VERSION;
}

/** The persona's platform when it is one we can present, else this machine's. */
function platformKey(profile) {
  const named = profile?.navigator?.platform || '';
  if (Object.hasOwn(PLATFORMS, named)) return named;
  return Object.hasOwn(HOST_PLATFORMS, process.platform) ? HOST_PLATFORMS[process.platform] : 'Linux x86_64';
}

/**
 * Chrome's brand list for a major version, in Chrome's order with Chrome's
 * GREASE entry. Both are seeded by the major version, so a hardcoded list is
 * right for exactly one release and a tell on every other.
 */
function brandsFor(major) {
  const seed = Number(major);
  const name = `Not${GREASE_CHARS[seed % GREASE_CHARS.length]}A${GREASE_CHARS[(seed + 1) % GREASE_CHARS.length]}Brand`;
  const grease = { brand: name, version: GREASE_VERSIONS[seed % GREASE_VERSIONS.length] };
  const named = [grease, { brand: 'Chromium', version: major }, { brand: 'Google Chrome', version: major }];
  const list = [];
  [...BRAND_ORDERS[seed % BRAND_ORDERS.length]].forEach((slot, i) => (list[Number(slot)] = named[i]));
  return list;
}

/** The brands with full versions: Chrome's real one, and the GREASE brand's padded out. */
function fullVersionsOf(brands, full) {
  return brands.map((b) => ({ brand: b.brand, version: b.brand.startsWith('Not') ? `${b.version}.0.0.0` : full }));
}

/** An Apple GPU is Apple Silicon: arm, while navigator.platform stays MacIntel, as on a real M-series Mac. */
function architectureOf(key, profile) {
  const gpu = `${profile?.webgl?.renderer || ''} ${profile?.webgl?.unmaskedRenderer || ''}`;
  return key === 'MacIntel' && /Apple M/i.test(gpu) ? 'arm' : 'x86';
}

/** The `userAgentMetadata` of Emulation.setUserAgentOverride: everything navigator.userAgentData can say. */
function metadataFor(key, profile, full) {
  const brands = brandsFor(full.split('.')[0]);
  const versions = { brands, fullVersionList: fullVersionsOf(brands, full), fullVersion: full };
  const device = { architecture: architectureOf(key, profile), bitness: '64', model: '', mobile: false, wow64: false };
  return { ...versions, ...PLATFORMS[key].hints, ...device };
}

/** A brand list as a structured header. */
const brandHeader = (list) => list.map((b) => `"${b.brand}";v="${b.version}"`).join(', ');

/** Client-hint headers that are a quoted string, and the metadata field each one says. */
const QUOTED_HINTS = {
  'sec-ch-ua-full-version': 'fullVersion',
  'sec-ch-ua-platform': 'platform',
  'sec-ch-ua-platform-version': 'platformVersion',
  'sec-ch-ua-arch': 'architecture',
  'sec-ch-ua-bitness': 'bitness',
  'sec-ch-ua-model': 'model',
};

/** Client-hint headers that are a boolean: a desktop browser is neither mobile nor WOW64. */
const BOOLEAN_HINTS = { 'sec-ch-ua-mobile': '?0', 'sec-ch-ua-wow64': '?0' };

/** The client-hint request headers, by lowercase name, saying what the metadata says. */
function hintHeaders(meta) {
  const quoted = Object.entries(QUOTED_HINTS).map(([header, field]) => [header, `"${meta[field]}"`]);
  const lists = {
    'sec-ch-ua': brandHeader(meta.brands),
    'sec-ch-ua-full-version-list': brandHeader(meta.fullVersionList),
  };
  return { ...lists, ...Object.fromEntries(quoted), ...BOOLEAN_HINTS };
}

/**
 * The identity of `profile` (null: no persona yet, so this machine's platform):
 * `userAgent` for the session, `override` for Emulation.setUserAgentOverride,
 * and `hints` for the request headers. The UA carries the reduced version
 * (`major.0.0.0`), as Chrome has sent since 101; the full one lives in the hints.
 */
function personaIdentity(profile, defaultUA) {
  const full = engineVersion(defaultUA);
  const key = platformKey(profile);
  const userAgent = userAgentFor(key, full.split('.')[0]);
  const userAgentMetadata = metadataFor(key, profile, full);
  const override = { userAgent, platform: key, userAgentMetadata, ...acceptLanguageOf(profile) };
  return { userAgent, override, hints: hintHeaders(userAgentMetadata) };
}

/**
 * The persona's languages for the override, which sets navigator.languages and the
 * Accept-Language header together (Chrome adds the q-values). Without it the header
 * was the app's single locale, "en-US", where Chrome sends "en-US,en;q=0.9".
 */
function acceptLanguageOf(profile) {
  const languages = profile?.navigator?.languages;
  if (!Array.isArray(languages) || !languages.length) return {};
  return { acceptLanguage: withBaseLanguage(languages).join(',') };
}

/** Chrome lists the base language after a lone regional one ("en-US" alone is a device captured from Electron, which lists only its locale). */
function withBaseLanguage(languages) {
  const [first] = languages;
  return languages.length === 1 && first.includes('-') ? [first, first.split('-')[0]] : languages;
}

/** Chrome's frozen UA string on a platform: only the major version moves. */
function userAgentFor(key, major) {
  return `Mozilla/5.0 (${PLATFORMS[key].os}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${major}.0.0.0 Safari/537.36`;
}

module.exports = { personaIdentity, brandsFor };
