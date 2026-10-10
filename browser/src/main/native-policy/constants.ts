/** Bounds shared by policy preflight; the native engine remains the final authority. */
/** Maximum locale or timezone length accepted by the native engine. */
export const MAX_POLICY_TEXT = 128;
/** Smallest valid web-exposed logical processor count. */
export const MIN_PROCESSORS = 1;
/** Largest processor count supported by the immutable native API. */
export const MAX_PROCESSORS = 256;
/** This subset deliberately rejects fields it cannot protect. */
export const POLICY_FIELDS = [
  'timeZone',
  'locale',
  'hardwareConcurrency',
  'languages',
  'platform',
  'userAgent',
  'userAgentMetadata',
] as const;
/** Required native methods are checked together before any mutation. */
export const POLICY_METHODS = [
  '_getOyaSessionPolicy',
  '_setOyaTimeZone',
  '_setOyaHardwareConcurrency',
  '_setOyaLocale',
  '_setOyaPlatform',
  '_setOyaUserAgent',
  '_setOyaUserAgentMetadata',
] as const;
/** Native readback revision includes language preferences and default request headers. */
export const POLICY_VERSION = 1;

/** Exact legacy desktop navigator strings implemented by the native engine. */
export const NATIVE_PLATFORMS = ['MacIntel', 'Win32', 'Linux x86_64'] as const;
/** Maximum printable native User-Agent length. */
export const MAX_USER_AGENT = 1024;
/** Maximum native brand count. */
export const MAX_BRANDS = 8;
/** Native form-factor vocabulary and maximum count. */
export const FORM_FACTORS = ['Desktop', 'Automotive', 'Mobile', 'Tablet', 'XR', 'EInk', 'Watch'] as const;
/** Exact metadata schema prevents silent loss of requested identity fields. */
export const METADATA_FIELDS = [
  'brands',
  'fullVersionList',
  'fullVersion',
  'platform',
  'platformVersion',
  'architecture',
  'model',
  'mobile',
  'bitness',
  'wow64',
  'formFactors',
] as const;
