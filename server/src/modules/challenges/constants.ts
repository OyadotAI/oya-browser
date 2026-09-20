/**
 * Every number the challenge solvers run on, by name: timeouts, poll intervals,
 * limits and the bit arithmetic of TOTP.
 */

/** Milliseconds per second, for unix-second timestamps and token lifetimes. */
export const MS_PER_SECOND = 1000;

// ── CAPTCHA ──

/** How long an external solver gets, unless OYA_CAPTCHA_TIMEOUT_MS says otherwise. */
export const DEFAULT_CAPTCHA_TIMEOUT_MS = 120_000;
/** One request to the solver API. */
export const CAPTCHA_REQUEST_TIMEOUT_MS = 30_000;
/** Pause between polls for a solver's answer. */
export const CAPTCHA_POLL_MS = 3000;
/**
 * The smallest a challenge widget can be drawn and still be one a person is being
 * asked to do: a reCAPTCHA checkbox is about 300x78, Turnstile 300x65. Anything
 * smaller is the badge or the scoring frame a site keeps on every page.
 */
export const MIN_CHALLENGE_WIDTH_PX = 100;
/** The same floor for height. */
export const MIN_CHALLENGE_HEIGHT_PX = 30;

// ── Mailbox ──

/** A cached access token is renewed this long before it expires. */
export const TOKEN_REFRESH_MARGIN_MS = 60_000;
/** One request to a mailbox or token endpoint. */
export const MAILBOX_REQUEST_TIMEOUT_MS = 15_000;
/** Assumed access-token lifetime when the provider does not say (seconds). */
export const DEFAULT_TOKEN_LIFETIME_S = 3600;
/** Characters of a provider's error body kept in the message. */
export const ERROR_DETAIL_CHARS = 200;

// ── Reading a page ──

/** How long a detector waits for a page that is still loading to answer. */
export const PAGE_READY_MS = 10_000;
/** Pause between attempts to read a page that is still loading. */
export const PAGE_READY_POLL_MS = 200;

// ── Login ──

/** Sign-in attempts per browser and site before stopping short of a lockout. */
export const MAX_LOGIN_ATTEMPTS = 2;
/** How long a submitted sign-in form gets to go away. */
export const LOGIN_CONFIRM_MS = 20_000;
/** Pause between checks of a submitted sign-in form. */
export const LOGIN_POLL_MS = 500;

// ── MFA ──

/** How long a relay or mailbox is polled for a code, unless its config says otherwise. */
export const DEFAULT_RELAY_TIMEOUT_MS = 90_000;
/** One request to an SMS or email relay. */
export const RELAY_REQUEST_TIMEOUT_MS = 15_000;
/** Pause between relay polls. */
export const RELAY_POLL_MS = 5000;
/** How long a submitted code gets for the challenge to go away. */
export const MFA_CONFIRM_MS = 10_000;
/** Pause between checks of a submitted code. */
export const MFA_POLL_MS = 250;
/** Characters of a message handed to the code extractor. */
export const MAX_MESSAGE_CHARS = 4000;

// ── TOTP (RFC 6238) ──

/** Bits one base32 character carries. */
export const BASE32_BITS = 5;
/** Radix of a bit string. */
export const BINARY = 2;
/** Bytes in the HOTP counter. */
export const TOTP_COUNTER_BYTES = 8;
/** Low nibble of the last HMAC byte: the dynamic truncation offset. */
export const TOTP_OFFSET_MASK = 0x0f;
/** Drops the sign bit of the truncated 32-bit value. */
export const TOTP_CODE_MASK = 0x7fffffff;
/** Codes are decimal digits. */
export const DECIMAL = 10;
