/**
 * What the observer keeps, and for how long.
 */

/** Console entries kept per browser. Enough to cover a page's own error burst. */
export const CONSOLE_MAX = 500;

/** Network records kept per browser. A portal page can make hundreds of requests. */
export const NETWORK_MAX = 1000;

/** Longest message or url text kept; the rest is a page's own noise. */
export const TEXT_MAX = 2000;

/** Electron reports this as the "error" of a request that was fine. */
export const NO_ERROR = 'net::OK';

/** The first HTTP status that means the server refused the request. */
export const HTTP_ERROR_FLOOR = 400;

/**
 * How long a caller's pattern may run over one read's entries. The pattern is
 * theirs and the text is the page's, and a regexp has no limit of its own: one
 * that backtracks forever would hold the main process, and with it the whole app.
 */
export const PATTERN_BUDGET_MS = 100;

/** Electron's console levels, by the number it reports. */
export const LEVELS: readonly string[] = ['debug', 'info', 'warning', 'error'];

/** How many entries a read answers when the caller names no limit. */
export const DEFAULT_READ_LIMIT = 100;

/** What a caller is told when their pattern ran out of time: a quiet "nothing matched" would read as "nothing failed". */
export const PATTERN_TOO_SLOW = 'The pattern took too long to match. Use a simpler pattern, or plain text.';
