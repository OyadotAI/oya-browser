/**
 * The browser a persona says it is, from one place: the User-Agent string, the
 * page-side override (navigator.userAgent and navigator.userAgentData) and the
 * client-hint headers. They used to be built apart, and only the headers claimed
 * Google Chrome while page JavaScript saw Electron's own brands and the host OS.
 * Building them together avoids that inconsistency; it does not establish why
 * Google rejects a sign-in or guarantee that Google accepts an embedded browser.
 *
 * The version is always the engine's own. Claiming a newer Chrome than the one
 * running (a mirrored profile's, say) is caught by testing for features that
 * version has, so it is never claimed.
 */
import {
  FALLBACK_CHROME_VERSION,
  PLATFORMS,
  HOST_PLATFORMS,
  DEFAULT_PLATFORM,
  GREASE_CHARS,
  GREASE_VERSIONS,
  BRAND_ORDERS,
  QUOTED_HINTS,
  BOOLEAN_HINTS,
} from './constants.ts';

/** The parts of a persona's device the identity reads. */
export interface PersonaProfile {
  /** What navigator says. */
  navigator?: ProfileNavigator;
  /** What WebGL says, for telling Apple Silicon apart. */
  webgl?: ProfileWebgl;
}

/** The persona's navigator, as far as the identity reads it. */
export interface ProfileNavigator {
  /** navigator.platform. */
  platform?: string;
  /** navigator.languages; checked before use. */
  languages?: unknown;
}

/** The persona's WebGL renderer names. */
export interface ProfileWebgl {
  /** The renderer a page reads. */
  renderer?: string;
  /** The real GPU name, which a mirrored device keeps. */
  unmaskedRenderer?: string;
}

/** The languages part of the override. */
interface AcceptLanguage {
  /** navigator.languages and Accept-Language, comma-joined. */
  acceptLanguage?: string;
}

/** One entry of a brand list. */
export interface Brand {
  /** The brand's name. */
  brand: string;
  /** Its version: the major one, or the full one in a full-version list. */
  version: string;
}

/** Everything navigator.userAgentData can say: Emulation.setUserAgentOverride's `userAgentMetadata`. */
export interface UserAgentMetadata {
  /** Brands with major versions. */
  brands: Brand[];
  /** Brands with full versions. */
  fullVersionList: Brand[];
  /** The engine's full version. */
  fullVersion: string;
  /** The OS name. */
  platform: string;
  /** The OS version. */
  platformVersion: string;
  /** arm or x86. */
  architecture: string;
  /** Always 64. */
  bitness: string;
  /** Empty on a desktop. */
  model: string;
  /** Never, on a desktop. */
  mobile: boolean;
  /** Never, on a 64-bit browser. */
  wow64: boolean;
}

/** Emulation.setUserAgentOverride's parameters. */
export interface UserAgentOverride {
  /** The UA string. */
  userAgent: string;
  /** navigator.platform. */
  platform: string;
  /** navigator.userAgentData. */
  userAgentMetadata: UserAgentMetadata;
  /** navigator.languages and the Accept-Language header, when the persona names languages. */
  acceptLanguage?: string;
}

/** A persona's identity, ready for the session, the page and the request headers. */
export interface PersonaIdentity {
  /** The session's UA string. */
  userAgent: string;
  /** For Emulation.setUserAgentOverride. */
  override: UserAgentOverride;
  /** The client-hint request headers, by lowercase name. */
  hints: Record<string, string>;
}

/** The engine's Chrome version: Electron's own, else the one in `defaultUA`, else a known one. */
function engineVersion(defaultUA = ''): string {
  return process.versions.chrome || defaultUA.match(/Chrome\/([\d.]+)/)?.[1] || FALLBACK_CHROME_VERSION;
}

/** The persona's platform when it is one we can present, else this machine's. */
function platformKey(profile: PersonaProfile | null | undefined): string {
  const named = profile?.navigator?.platform || '';
  if (Object.hasOwn(PLATFORMS, named)) return named;
  return Object.hasOwn(HOST_PLATFORMS, process.platform) ? HOST_PLATFORMS[process.platform] : DEFAULT_PLATFORM;
}

/** The GREASE brand's name for a seed. */
const greaseName = (seed: number): string =>
  `Not${GREASE_CHARS[seed % GREASE_CHARS.length]}A${GREASE_CHARS[(seed + 1) % GREASE_CHARS.length]}Brand`;

/**
 * Chrome's brand list for a major version, in Chrome's order with Chrome's
 * GREASE entry. Both are seeded by the major version, so a hardcoded list is
 * right for exactly one release and a tell on every other.
 */
export function brandsFor(major: string): Brand[] {
  const seed = Number(major);
  const grease = { brand: greaseName(seed), version: GREASE_VERSIONS[seed % GREASE_VERSIONS.length] };
  const named = [grease, { brand: 'Chromium', version: major }, { brand: 'Google Chrome', version: major }];
  const list: Brand[] = [];
  [...BRAND_ORDERS[seed % BRAND_ORDERS.length]].forEach((slot, i) => (list[Number(slot)] = named[i]));
  return list;
}

/** The brands with full versions: Chrome's real one, and the GREASE brand's padded out. */
function fullVersionsOf(brands: Brand[], full: string): Brand[] {
  return brands.map((b) => ({ brand: b.brand, version: b.brand.startsWith('Not') ? `${b.version}.0.0.0` : full }));
}

/** An Apple GPU is Apple Silicon: arm, while navigator.platform stays MacIntel, as on a real M-series Mac. */
function architectureOf(key: string, profile: PersonaProfile | null | undefined): string {
  const gpu = `${profile?.webgl?.renderer || ''} ${profile?.webgl?.unmaskedRenderer || ''}`;
  return key === 'MacIntel' && /Apple M/i.test(gpu) ? 'arm' : 'x86';
}

/** The `userAgentMetadata` of Emulation.setUserAgentOverride: everything navigator.userAgentData can say. */
function metadataFor(key: string, profile: PersonaProfile | null | undefined, full: string): UserAgentMetadata {
  const brands = brandsFor(full.split('.')[0]);
  const versions = { brands, fullVersionList: fullVersionsOf(brands, full), fullVersion: full };
  const device = { architecture: architectureOf(key, profile), bitness: '64', model: '', mobile: false, wow64: false };
  return { ...versions, ...PLATFORMS[key].hints, ...device };
}

/** A brand list as a structured header. */
const brandHeader = (list: Brand[]): string => list.map((b) => `"${b.brand}";v="${b.version}"`).join(', ');

/** The client-hint request headers, by lowercase name, saying what the metadata says. */
function hintHeaders(meta: UserAgentMetadata): Record<string, string> {
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
export function personaIdentity(profile: PersonaProfile | null | undefined, defaultUA?: string): PersonaIdentity {
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
function acceptLanguageOf(profile: PersonaProfile | null | undefined): AcceptLanguage {
  const languages = profile?.navigator?.languages;
  if (!Array.isArray(languages) || !languages.length) return {};
  return { acceptLanguage: withBaseLanguage(languages).join(',') };
}

/** Chrome lists the base language after a lone regional one ("en-US" alone is a device captured from Electron, which lists only its locale). */
function withBaseLanguage(languages: string[]): string[] {
  const [first] = languages;
  return languages.length === 1 && first.includes('-') ? [first, first.split('-')[0]] : languages;
}

/** Chrome's frozen UA string on a platform: only the major version moves. */
function userAgentFor(key: string, major: string): string {
  return `Mozilla/5.0 (${PLATFORMS[key].os}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${major}.0.0.0 Safari/537.36`;
}
