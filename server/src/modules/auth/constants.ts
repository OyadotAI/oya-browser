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

/** Cloudflare's endpoint that says whether a Turnstile token is genuine. */
export const TURNSTILE_VERIFY_URL = 'https://challenges.cloudflare.com/turnstile/v0/siteverify';
/** How long to wait for Cloudflare before letting the request through. */
export const TURNSTILE_TIMEOUT_MS = 10_000;

/** Leading hex zeros sha256(challenge + nonce) must have: about a million hashes, a second or two for one agent. */
export const AGENT_POW_ZEROS = 5;
/** How long an agent has to solve a challenge and sign up with it. */
export const AGENT_CHALLENGE_TTL_MS = 600_000;
/** Longest nonce accepted, so a signup cannot make the server hash megabytes. */
export const AGENT_NONCE_MAX_CHARS = 64;
/** Longest email accepted from an agent. */
export const AGENT_EMAIL_MAX_CHARS = 254;
/** How recently an account must have been made for a Google or GitHub sign-in to count as its sign-up: ten minutes. */
export const NEW_ACCOUNT_MS = 600_000;
