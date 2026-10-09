/** Native cookie limits bound external batches and returned metadata. */
export const COOKIE_LIMITS = {
  /** Never enqueue an unbounded sequence of native mutations. */
  batch: 128,
  /** Conservative byte bound; the native cookie manager remains authoritative. */
  bytes: 4096,
  /** Bound context export without truncating a successful response. */
  records: 4096,
  /** Protocol session-cookie expiration sentinel. */
  session: -1,
};
/** Actual engine enum values, not guessed cookie defaults. */
export const COOKIE_PRIORITY: Record<number, string> = { 0: 'Low', 1: 'Medium', 2: 'High' };
/** Actual native scheme provenance enum. */
export const COOKIE_SCHEME: Record<number, string> = { 0: 'Unset', 1: 'NonSecure', 2: 'Secure' };
/** Supported explicit SameSite semantics mapped to native API values. */
export const COOKIE_SAME_SITE = { Strict: 'strict', Lax: 'lax', None: 'no_restriction' } as const;
