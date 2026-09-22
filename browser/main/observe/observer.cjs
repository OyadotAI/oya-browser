/**
 * What the page said and what it fetched, collected where the page cannot see it.
 *
 * An agent that cannot read a console or a failed request cannot tell "the
 * server rejected my data" from "the page is broken", so it guesses, which is
 * the wrong failure mode for a portal that holds someone's medical record.
 *
 * Both sources are browser-process APIs, not CDP: `session.webRequest` (already
 * used by governance) and Electron's `console-message`. Neither enables
 * `Runtime`, `Log` or `Network`, which are the domains a page can detect, and
 * neither injects anything into the page. From the page's side nothing changes.
 *
 * Urls keep their origin and path but lose their query, because a portal puts
 * member ids and tokens there and this buffer is read back over the API.
 */

const vm = require('node:vm');
const {
  CONSOLE_MAX,
  NETWORK_MAX,
  TEXT_MAX,
  LEVELS,
  HTTP_ERROR_FLOOR,
  NO_ERROR,
  PATTERN_BUDGET_MS,
} = require('./constants.cjs');

/** A url without its query, fragment or credentials; anything unparsable is dropped whole. */
function safeUrl(raw) {
  try {
    const url = new URL(raw);
    Object.assign(url, { username: '', password: '', search: '', hash: '' });
    return url.toString().slice(0, TEXT_MAX);
  } catch {
    return '[unparsable url]';
  }
}

/** One console entry as it is kept: level named, text capped, source without its query. */
function consoleEntry({ level, message, line, sourceId }) {
  return {
    at: Date.now(),
    level: LEVELS[level] || String(level),
    message: String(message ?? '').slice(0, TEXT_MAX),
    source: safeUrl(sourceId || ''),
    line,
  };
}

/** One network record as it is kept: url without its query, error only when real. */
function networkEntry({ url, method, resourceType, statusCode, error }) {
  return {
    at: Date.now(),
    url: safeUrl(url),
    method,
    type: resourceType,
    status: statusCode ?? null,
    error: error && error !== NO_ERROR ? error : null,
  };
}

/** Whether this is traffic the page itself made, rather than a browser extension's. */
function isPageRequest(url) {
  return /^https?:\/\//i.test(String(url ?? ''));
}

/** Keeps at most `max` entries, dropping the oldest. */
function push(buffer, entry, max) {
  buffer.push(entry);
  if (buffer.length > max) buffer.shift();
}

/** The console and network rings for one browser. */
class Observer {
  /** Starts empty; nothing is collected until a session and tabs are watched. */
  constructor() {
    /** Console entries, oldest first. */
    this.console = [];
    /** Network records, oldest first. */
    this.network = [];
  }

  /** Records one console entry from a renderer. */
  addConsole({ level, message, line, sourceId }) {
    push(this.console, consoleEntry({ level, message, line, sourceId }), CONSOLE_MAX);
  }

  /**
   * Records one finished request. A request that succeeded carries no error,
   * whatever Electron called it, and only the page's own traffic is kept: an
   * extension failing to load its own icon is not an answer to "what failed?".
   */
  addRequest(request) {
    if (!isPageRequest(request.url)) return;
    push(this.network, networkEntry(request), NETWORK_MAX);
  }

  /** Console entries, newest first, optionally only one level or matching text. */
  readConsole({ level, pattern, limit = 100 } = {}) {
    const hit = matcher(
      pattern,
      this.console.map((e) => e.message),
    );
    return filtered(this.console, (e, i) => (!level || e.level === level) && hit(i), limit);
  }

  /**
   * Network records, newest first. `failedOnly` is the question an agent
   * actually has: which request did the server refuse?
   */
  readNetwork({ failedOnly = false, pattern, limit = 100 } = {}) {
    const failed = (e) => Boolean(e.error) || (e.status !== null && e.status >= HTTP_ERROR_FLOOR);
    const hit = matcher(
      pattern,
      this.network.map((e) => e.url),
    );
    return filtered(this.network, (e, i) => (!failedOnly || failed(e)) && hit(i), limit);
  }
}

/**
 * Which of `texts` the pattern matches, as a case-insensitive regexp. It runs in
 * a vm with a time budget, which is the one way to stop a regexp that backtracks
 * forever: the pattern is the caller's, the text is the page's, and this is the
 * main process. Throws when the pattern is invalid or runs out of time.
 */
function regexpHits(pattern, texts) {
  const sandbox = { re: new RegExp(pattern, 'i'), texts };
  return vm.runInNewContext('texts.map((text) => re.test(text))', sandbox, { timeout: PATTERN_BUDGET_MS });
}

/** What a caller is told when their pattern ran out of time: a quiet "nothing matched" would read as "nothing failed". */
const PATTERN_TOO_SLOW = 'The pattern took too long to match. Use a simpler pattern, or plain text.';

/**
 * Answers whether entry `i` matches: every entry with no pattern, and plain
 * text when the pattern is not a valid regexp. One that runs out of time is
 * refused instead, because matching it as text would quietly answer the wrong question.
 */
function matcher(pattern, texts) {
  if (!pattern) return () => true;
  try {
    const hits = regexpHits(pattern, texts);
    return (i) => hits[i];
  } catch (err) {
    if (err.code === 'ERR_SCRIPT_EXECUTION_TIMEOUT') throw new Error(PATTERN_TOO_SLOW);
    return (i) => String(texts[i]).includes(pattern);
  }
}

/** The newest `limit` entries that pass the test, newest first. */
function filtered(buffer, test, limit) {
  return buffer
    .filter(test)
    .slice(-Math.max(1, Math.min(limit, buffer.length || 1)))
    .reverse();
}

module.exports = { Observer, safeUrl };
