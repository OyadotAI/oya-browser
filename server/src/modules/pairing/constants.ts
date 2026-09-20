/**
 * Every number desktop pairing runs on, by name: code lifetime, code size and
 * how many codes may wait at once.
 */

/** Milliseconds per minute, for readable lifetimes. */
const MS_PER_MINUTE = 60_000;
/** Minutes a pairing code stays valid by default. */
const DEFAULT_TTL_MINUTES = 5;
/** A code lives five minutes unless OYA_PAIRING_TTL_MS says otherwise. */
const DEFAULT_TTL_MS = DEFAULT_TTL_MINUTES * MS_PER_MINUTE;
/** OYA_PAIRING_TTL_MS, or the default. */
export const TTL_MS = Number(process.env.OYA_PAIRING_TTL_MS) || DEFAULT_TTL_MS;
/** Codes are unauthenticated on claim, so guessing must be hopeless and slow. */
export const MAX_OUTSTANDING = 200;
/** Random bytes per code: 256 bits. */
export const CODE_BYTES = 32;
/** Shortest code string worth looking up. */
export const MIN_CODE_LENGTH = 16;
/** Longest code string worth looking up. */
export const MAX_CODE_LENGTH = 128;
