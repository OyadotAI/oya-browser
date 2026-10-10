/**
 * Every number the cookie sync with the server's pool runs on, by name: how
 * often a host is pulled, how long a navigation waits for it, and how local
 * changes are batched.
 */

/** A host's cookies are pulled from the pool at most this often. */
export const COOKIE_PULL_TTL_MS = 30_000;
/** A navigation never waits longer than this for a cookie pull. */
export const COOKIE_PULL_TIMEOUT_MS = 3000;
/** Local cookie changes are batched for this long before they are sent. */
export const COOKIE_FLUSH_MS = 2000;
/** A batch this large is sent at once rather than waiting out the timer. */
export const COOKIE_BATCH_MAX = 200;
/** Hosts remembered as recently pulled; past this the memory is cleared. */
export const COOKIE_PULLED_HOSTS_MAX = 500;

/** Bound profile origins before opening native observer pipes. */
export const STORAGE_ORIGINS_MAX = 1024;
/** Bound snapshots consistently with the patched storage engine. */
export const STORAGE_ENTRIES_MAX = 4096;
/** Maximum UTF-16 code units in one origin's snapshot. */
export const STORAGE_UNITS_MAX = 1_048_576;
/** A constantly mutating page must not hold profile flushing forever. */
export const STORAGE_CAPTURE_TIMEOUT_MS = 20_000;

/** A storage entry contains exactly a key and a value. */
export const STORAGE_PAIR_LENGTH = 2;

/** Batch native storage mutations without repeatedly scanning unchanged origins. */
export const STORAGE_FLUSH_MS = 2000;

/** Native rejection labels safe to count without exposing cookie names, domains or values. */
export const COOKIE_REJECTION_CODES = [
  'EXCLUDE_UNKNOWN_ERROR',
  'EXCLUDE_FAILURE_TO_STORE',
  'EXCLUDE_INVALID_DOMAIN',
  'EXCLUDE_INVALID_PREFIX',
  'EXCLUDE_INVALID_PATH',
  'EXCLUDE_SAMESITE_NONE_INSECURE',
  'EXCLUDE_NAME_VALUE_PAIR_EXCEEDS_MAX_SIZE',
  'EXCLUDE_ATTRIBUTE_VALUE_EXCEEDS_MAX_SIZE',
  'EXCLUDE_DOMAIN_NON_ASCII',
  'EXCLUDE_DISALLOWED_CHARACTER',
  'EXCLUDE_NO_COOKIE_CONTENT',
  'EXCLUDE_AMBIGUOUS_SERIALIZATION',
  'EXCLUDE_OVERWRITE_SECURE',
  'EXCLUDE_OVERWRITE_HTTP_ONLY',
  'EXCLUDE_USER_PREFERENCES',
  'EXCLUDE_DOMAIN_MISMATCH',
  'EXCLUDE_NOT_ON_PATH',
] as const;

/** Native refusal preserves an existing stronger local cookie instead of importing a weaker server copy. */
export const COOKIE_LOCAL_CONFLICT_CODES = ['EXCLUDE_OVERWRITE_SECURE', 'EXCLUDE_OVERWRITE_HTTP_ONLY'] as const;
