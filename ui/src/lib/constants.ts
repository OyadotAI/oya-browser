/**
 * Every number the shared console library runs on, by name: time units,
 * retry delays and display limits.
 */

/** Milliseconds in a second. */
export const MS_PER_SECOND = 1000;
/** Seconds in a minute. */
export const SECONDS_PER_MINUTE = 60;
/** Seconds in an hour. */
export const SECONDS_PER_HOUR = 3600;
/** Seconds in a day. */
export const SECONDS_PER_DAY = 86_400;
/** Anything younger than this reads as "now" rather than a count of seconds. */
export const JUST_NOW_SECONDS = 5;

/** Ids longer than this are shortened for tables. */
export const SHORT_ID_MAX = 14;
/** Characters of a shortened id that stay visible. */
export const SHORT_ID_KEEP = 8;

/** Wait before a dropped live-view stream asks for a new ticket. */
export const LIVE_RETRY_MS = 2000;

/** A session token is renewed this long before it expires. */
export const REFRESH_LEAD_SECONDS = 60;
/** Never schedule a token renewal sooner than this, even for a nearly expired token. */
export const MIN_REFRESH_DELAY_MS = 1000;

/** After Meta's pixel library loads, how long a sign-up event gets to leave before the page reloads. */
export const META_FLUSH_MS = 300;
/** The longest a sign-up waits on Meta's pixel, so a blocked or slow pixel never holds it up. */
export const META_MAX_WAIT_MS = 1500;

/** How long the site remembers where a visitor came from, so a download days later still says. */
export const SOURCE_MEMORY_DAYS = 30;
/** The longest source kept: a campaign name, never a page of text. */
export const SOURCE_MAX_LENGTH = 80;

/** Digits in a code from an authenticator app. */
export const TOTP_CODE_LENGTH = 6;
