/**
 * Every number and fixed string the auth module runs on, by name: key sizes,
 * input limits and session-cookie settings.
 */

/** Random bytes in a minted API key (32 base64url characters). */
export const KEY_BYTES = 24;
/** Leading characters of a key kept for display. */
export const KEY_PREFIX_CHARS = 8;
/** The Authorization scheme a token travels under. */
export const BEARER = 'Bearer ';
/** Longest project name taken from a key's label. */
export const MAX_PROJECT_NAME = 100;
/** Longest display name a person can give themselves. */
export const MAX_DISPLAY_NAME = 100;
/** Shortest password accepted at signup. */
export const MIN_PASSWORD_CHARS = 8;

/** Milliseconds in a day. */
const DAY_MS = 86_400_000;
/** Days a refresh cookie lives. */
const SESSION_DAYS = 30;
/** How long the session cookies last. */
export const SESSION_MAX_AGE_MS = SESSION_DAYS * DAY_MS;
