/** Bounds shared by policy preflight; the native engine remains the final authority. */
/** Maximum locale or timezone length accepted by the native engine. */
export const MAX_POLICY_TEXT = 128;
/** Smallest valid web-exposed logical processor count. */
export const MIN_PROCESSORS = 1;
/** Largest processor count supported by the immutable native API. */
export const MAX_PROCESSORS = 256;
/** This subset deliberately rejects fields it cannot protect. */
export const POLICY_FIELDS = ['timeZone', 'locale', 'hardwareConcurrency', 'languages', 'platform'] as const;
/** Required native methods are checked together before any mutation. */
export const POLICY_METHODS = [
  '_getOyaSessionPolicy',
  '_setOyaTimeZone',
  '_setOyaHardwareConcurrency',
  '_setOyaLocale',
  '_setOyaPlatform',
] as const;
/** Native readback revision includes language preferences and default request headers. */
export const POLICY_VERSION = 1;

/** Exact legacy desktop navigator strings implemented by the native engine. */
export const NATIVE_PLATFORMS = ['MacIntel', 'Win32', 'Linux x86_64'] as const;
