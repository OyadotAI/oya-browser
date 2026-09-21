/**
 * What the observer keeps, and for how long.
 */

/** Console entries kept per browser. Enough to cover a page's own error burst. */
const CONSOLE_MAX = 500;

/** Network records kept per browser. A portal page can make hundreds of requests. */
const NETWORK_MAX = 1000;

/** Longest message or url text kept; the rest is a page's own noise. */
const TEXT_MAX = 2000;

/** Electron reports this as the "error" of a request that was fine. */
const NO_ERROR = 'net::OK';

/** The first HTTP status that means the server refused the request. */
const HTTP_ERROR_FLOOR = 400;

/** Electron's console levels, by the number it reports. */
const LEVELS = ['debug', 'info', 'warning', 'error'];

module.exports = { CONSOLE_MAX, NETWORK_MAX, TEXT_MAX, LEVELS, HTTP_ERROR_FLOOR, NO_ERROR };
