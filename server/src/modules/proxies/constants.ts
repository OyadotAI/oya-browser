/**
 * Every number and fixed value the proxy pool runs on, by name. Values an
 * operator may tune come from the environment.
 */

/** First cooldown after a failure; doubles per consecutive failure. */
export const COOLDOWN_MS = 60_000;
/** Longest a failed proxy is skipped (15 minutes). */
export const MAX_COOLDOWN_MS = 900_000;
/** Cooldown grows by this factor per consecutive failure. */
export const BACKOFF_FACTOR = 2;
/** Health checks give up after this long, unless OYA_PROXY_CHECK_TIMEOUT_MS says otherwise. */
const DEFAULT_CHECK_TIMEOUT_MS = 10_000;
/** OYA_PROXY_CHECK_TIMEOUT_MS, or the default. */
export const CHECK_TIMEOUT_MS = Number(process.env.OYA_PROXY_CHECK_TIMEOUT_MS) || DEFAULT_CHECK_TIMEOUT_MS;
/** Where a health check asks for its exit IP, unless OYA_PROXY_CHECK_URL says otherwise. */
export const DEFAULT_CHECK_URL = 'https://api.ipify.org?format=json';

/** Random bytes in a proxy id ("px-" plus hex). */
export const ID_BYTES = 6;
/** URL schemes a proxy may use. */
export const PROXY_PROTOCOLS = ['http:', 'https:', 'socks5:', 'socks:'];

/** Default port for an https target or proxy. */
export const HTTPS_PORT = 443;
/** Default port for an http target or proxy. */
export const HTTP_PORT = 80;
/** The only answer a CONNECT or a check GET counts as success. */
export const HTTP_OK = 200;

/** Characters of a geo code that name the country ("US-CA" → "US"). */
export const COUNTRY_CODE_CHARS = 2;
/** Hex characters of the sticky residential session id. */
export const SESSION_ID_CHARS = 16;
/** Country a residential exit uses when neither the persona nor the operator names one. */
export const DEFAULT_RESIDENTIAL_GEO = 'US';

/** Timezone regions (the part before the "/") plausible for each exit country. */
export const PLAUSIBLE_REGIONS: Record<string, string[]> = {
  US: ['America', 'Pacific'],
  CA: ['America'],
  GB: ['Europe'],
  DE: ['Europe'],
  FR: ['Europe'],
  NL: ['Europe'],
  ES: ['Europe'],
  IT: ['Europe'],
  AU: ['Australia'],
  JP: ['Asia'],
  SG: ['Asia'],
  IN: ['Asia'],
  BR: ['America'],
};
