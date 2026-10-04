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
import vm from 'node:vm';
import {
  CONSOLE_MAX,
  NETWORK_MAX,
  TEXT_MAX,
  LEVELS,
  HTTP_ERROR_FLOOR,
  NO_ERROR,
  PATTERN_BUDGET_MS,
  DEFAULT_READ_LIMIT,
  PATTERN_TOO_SLOW,
} from './constants.ts';

/** A console message as a renderer reports it. */
export interface ConsoleInput {
  /** Electron's level: a name, or the number older events carried. */
  level: string | number;
  /** The text logged. */
  message?: unknown;
  /** The line it was logged from. */
  line?: number;
  /** The script it was logged from. */
  sourceId?: string;
}

/** A finished or failed request as the session reports it. */
export interface RequestInput {
  /** Where it went. */
  url: string;
  /** Its HTTP method. */
  method?: string;
  /** What kind of resource it fetched. */
  resourceType?: string;
  /** The response status, when one came. */
  statusCode?: number;
  /** Why it failed, when it did. */
  error?: string;
}

/** One console entry as it is kept. */
export interface ConsoleEntry {
  /** When it was kept, in epoch milliseconds. */
  at: number;
  /** debug, info, warning or error. */
  level: string;
  /** The text, capped. */
  message: string;
  /** The script's url without its query. */
  source: string;
  /** The line it came from. */
  line: number | undefined;
}

/** One network record as it is kept. */
export interface NetworkEntry {
  /** When it was kept, in epoch milliseconds. */
  at: number;
  /** The url without its query. */
  url: string;
  /** Its HTTP method. */
  method: string | undefined;
  /** What kind of resource it fetched. */
  type: string | undefined;
  /** The response status, or null when none came. */
  status: number | null;
  /** Why it failed, or null when it did not. */
  error: string | null;
}

/** What readConsole() is asked. */
export interface ConsoleQuery {
  /** Only this level. */
  level?: string;
  /** Only messages matching this regexp, or containing this text. */
  pattern?: string;
  /** At most this many entries. */
  limit?: number;
}

/** What readNetwork() is asked. */
export interface NetworkQuery {
  /** Only requests that failed or were refused. */
  failedOnly?: boolean;
  /** Only urls matching this regexp, or containing this text. */
  pattern?: string;
  /** At most this many records. */
  limit?: number;
}

/** A url without its query, fragment or credentials; anything unparsable is dropped whole. */
export function safeUrl(raw: string): string {
  try {
    const url = new URL(raw);
    Object.assign(url, { username: '', password: '', search: '', hash: '' });
    return url.toString().slice(0, TEXT_MAX);
  } catch {
    return '[unparsable url]';
  }
}

/** LEVELS looked up by whatever Electron reported: a number names a level, a name finds nothing. */
const LEVEL_NAMES = LEVELS as unknown as Readonly<Record<string | number, string | undefined>>;

/** The level's name: Electron's number named, a name kept as is. */
const levelName = (level: string | number): string => LEVEL_NAMES[level] || String(level);

/** One console entry as it is kept: level named, text capped, source without its query. */
function consoleEntry({ level, message, line, sourceId }: ConsoleInput): ConsoleEntry {
  return {
    at: Date.now(),
    level: levelName(level),
    message: String(message ?? '').slice(0, TEXT_MAX),
    source: safeUrl(sourceId || ''),
    line,
  };
}

/** One network record as it is kept: url without its query, error only when real. */
function networkEntry({ url, method, resourceType, statusCode, error }: RequestInput): NetworkEntry {
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
function isPageRequest(url: unknown): boolean {
  return /^https?:\/\//i.test(String(url ?? ''));
}

/** Keeps at most `max` entries, dropping the oldest. */
function push<T>(buffer: T[], entry: T, max: number): void {
  buffer.push(entry);
  if (buffer.length > max) buffer.shift();
}

/** Whether a record is a failure: no answer, or a refusal. */
const failed = (e: NetworkEntry): boolean => Boolean(e.error) || (e.status !== null && e.status >= HTTP_ERROR_FLOOR);

/** The console and network rings for one browser. */
export class Observer {
  /** Console entries, oldest first. */
  readonly console: ConsoleEntry[] = [];
  /** Network records, oldest first. */
  readonly network: NetworkEntry[] = [];

  /** Records one console entry from a renderer. */
  addConsole({ level, message, line, sourceId }: ConsoleInput): void {
    push(this.console, consoleEntry({ level, message, line, sourceId }), CONSOLE_MAX);
  }

  /**
   * Records one finished request. A request that succeeded carries no error,
   * whatever Electron called it, and only the page's own traffic is kept: an
   * extension failing to load its own icon is not an answer to "what failed?".
   */
  addRequest(request: RequestInput): void {
    if (!isPageRequest(request.url)) return;
    push(this.network, networkEntry(request), NETWORK_MAX);
  }

  /** Console entries, newest first, optionally only one level or matching text. */
  readConsole({ level, pattern, limit = DEFAULT_READ_LIMIT }: ConsoleQuery = {}): ConsoleEntry[] {
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
  readNetwork({ failedOnly = false, pattern, limit = DEFAULT_READ_LIMIT }: NetworkQuery = {}): NetworkEntry[] {
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
function regexpHits(pattern: string, texts: string[]): boolean[] {
  const sandbox = { re: new RegExp(pattern, 'i'), texts };
  return vm.runInNewContext('texts.map((text) => re.test(text))', sandbox, { timeout: PATTERN_BUDGET_MS });
}

/** Whether a thrown value is the vm's time budget running out. */
const timedOut = (err: unknown): boolean =>
  (err as NodeJS.ErrnoException | null)?.code === 'ERR_SCRIPT_EXECUTION_TIMEOUT';

/**
 * Answers whether entry `i` matches: every entry with no pattern, and plain
 * text when the pattern is not a valid regexp. One that runs out of time is
 * refused instead, because matching it as text would quietly answer the wrong question.
 */
function matcher(pattern: string | undefined, texts: string[]): (i: number) => boolean {
  if (!pattern) return () => true;
  try {
    const hits = regexpHits(pattern, texts);
    return (i) => hits[i];
  } catch (err) {
    if (timedOut(err)) throw new Error(PATTERN_TOO_SLOW, { cause: err });
    return (i) => String(texts[i]).includes(pattern);
  }
}

/** The newest `limit` entries that pass the test, newest first. */
function filtered<T>(buffer: T[], test: (e: T, i: number) => boolean, limit: number): T[] {
  return buffer
    .filter(test)
    .slice(-Math.max(1, Math.min(limit, buffer.length || 1)))
    .reverse();
}
