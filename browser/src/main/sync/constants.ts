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
