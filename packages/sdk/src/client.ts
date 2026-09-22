/**
 * The one HTTP path. Everything else in this package is a wrapper over it: it
 * adds the bearer key, encodes the body, applies the timeout and turns a
 * failed answer into an OyaError. `createHttp` builds it from the client's
 * options and the environment.
 */
import { OyaError, refusal } from './errors.js';
import { savedConfig } from './cli-config.js';
import {
  CODE_POINT_DIGITS,
  DEFAULT_BASE_URL,
  DEFAULT_TIMEOUT_MS,
  DEL,
  HEX,
  LATIN1_MAX,
  SPACE,
  TAB,
} from './constants.js';
import type { OyaOptions } from './types/index.js';

/** Where the key and the address came from, so a message names the one to fix. */
export interface Origin {
  /** The key's source, as a message names it: "the apiKey option", "OYA_API_KEY" or "~/.oya/config.json". */
  apiKeyFrom: string;
  /** The address's source, named the same way, or "default" for the hosted one. */
  baseUrlFrom: string;
  /** Whether a key is also saved by `oya login`, which an environment key overrides. */
  savedKey: boolean;
}

/** The origin assumed when an Http is built by hand. */
const OPTIONS_ORIGIN: Origin = { apiKeyFrom: 'the apiKey option', baseUrlFrom: 'the baseUrl option', savedKey: false };

/** The one HTTP path. Everything else in this package is a wrapper over it. */
export class Http {
  /** Stores where to call, with which key, how long to wait, which fetch to use, and where the key and address came from. */
  constructor(
    readonly baseUrl: string,
    readonly apiKey: string,
    private readonly timeoutMs: number,
    private readonly fetchImpl: typeof globalThis.fetch,
    readonly origin: Origin = OPTIONS_ORIGIN,
  ) {}

  /** Sends one request and returns the parsed answer, or throws an OyaError when it failed. */
  async request<T>(
    method: string,
    path: string,
    body?: unknown,
    timeoutMs = this.timeoutMs,
    headers: Record<string, string> = {},
  ): Promise<T> {
    const res = await this.send(`${this.baseUrl}${path}`, this.init(method, body, timeoutMs, headers), timeoutMs);
    return readAnswer<T>(res, `${method} ${path}`, this);
  }

  /** The fetch options for one call, with this client's key on them. */
  private init(method: string, body: unknown, timeoutMs: number, headers: Record<string, string>): RequestInit {
    return requestInit(method, body, timeoutMs, { Authorization: `Bearer ${this.apiKey}`, ...headers });
  }

  /**
   * fetch's own failures say only "fetch failed" or "The operation was
   * aborted", which leaves a reader guessing at the address, the port and
   * whether anything is listening. Say which it was.
   */
  private async send(url: string, init: RequestInit, timeoutMs: number): Promise<Response> {
    try {
      return await this.fetchImpl(url, init);
    } catch (err) {
      throw noAnswer(this, timeoutMs, err);
    }
  }
}

/** A fetch failure that is a time limit running out rather than a refused connection. */
const timedOut = (err: unknown) =>
  (err as Error)?.name === 'TimeoutError' || /abort/i.test(String((err as Error)?.message));

/**
 * The system's reason for a failed connection (ECONNREFUSED, ENOTFOUND), which
 * fetch keeps on its cause, or on the first of several when a name resolved twice.
 */
function causeOf(err: unknown): string | undefined {
  const cause = (err as FetchFailure)?.cause;
  return cause?.code ?? cause?.errors?.[0]?.code;
}

/** A system error's code, as Node sets it. */
interface SystemError {
  /** ECONNREFUSED, ENOTFOUND and the like. */
  code?: string;
}

/** What fetch throws when it could not connect. */
interface FetchFailure {
  /** The system error, or several when a name resolved to more than one address. */
  cause?: SystemError & {
    /** Each address's own error. */
    errors?: SystemError[];
  };
}

/** Status 0 with a code a caller can branch on, and the system's reason when there is one. */
function noAnswer(http: Http, timeoutMs: number, err: unknown): OyaError {
  const code = timedOut(err) ? 'timeout' : 'unreachable';
  const body = { error: String((err as Error)?.message), code, ...(causeOf(err) ? { cause: causeOf(err) } : {}) };
  return new OyaError(unreachable(http, timeoutMs, code), 0, body);
}

/** Why the request never got an answer, in words that name the address and where it came from. */
function unreachable({ baseUrl, origin }: Http, timeoutMs: number, code: string): string {
  if (code === 'timeout')
    return `No answer from ${baseUrl} within ${timeoutMs}ms. Is it reachable, and is the call this slow?`;
  if (origin.baseUrlFrom === 'default') return `Could not reach ${baseUrl}. Check your connection.`;
  return `Could not reach ${baseUrl}. Is the server running, and is ${origin.baseUrlFrom} right?`;
}

/** An error answer's body, when the server sent JSON. */
interface ErrorBody {
  /** The server's message. */
  error?: string;
}

/** The fetch options: the given headers, and a JSON body when there is one. */
function requestInit(method: string, body: unknown, timeoutMs: number, headers: Record<string, string>): RequestInit {
  const json: Record<string, string> = body === undefined ? {} : { 'Content-Type': 'application/json' };
  const payload = body === undefined ? undefined : JSON.stringify(body);
  return { method, headers: { ...headers, ...json }, body: payload, signal: AbortSignal.timeout(timeoutMs) };
}

/** The parsed answer, or an OyaError when the status says it failed. */
async function readAnswer<T>(res: Response, call: string, http: Http): Promise<T> {
  const payload = parseBody(await res.text());
  if (!res.ok) throw failure(call, res.status, payload, http);
  return payload as T;
}

/** JSON when it parses, the raw text when it does not, null when empty. */
function parseBody(text: string): unknown {
  try {
    return text ? JSON.parse(text) : null;
  } catch {
    return text;
  }
}

/** The error for a non-2xx answer: the server's message, or one naming the call. */
function failure(call: string, status: number, payload: unknown, http: Http): OyaError {
  const message = (payload as ErrorBody)?.error || `${call} failed (${status})`;
  return new OyaError(message === 'Invalid API key' ? invalidKey(http) : message, status, payload);
}

/** An invalid key, with where it came from; an environment key is said to override the saved one only when there is one. */
function invalidKey({ baseUrl, origin }: Http): string {
  const overrides = origin.apiKeyFrom === 'OYA_API_KEY' && origin.savedKey;
  const rest = overrides ? ', which overrides the one saved in ~/.oya/config.json' : '';
  return `Invalid API key for ${baseUrl}. The key came from ${origin.apiKeyFrom}${rest}.`;
}

/** The global scope, which has `process` in Node and not in a browser. */
interface MaybeProcess {
  /** Node's process, for its environment. */
  process?: {
    /** Environment variables. */
    env?: Record<string, string | undefined>;
  };
}

/** An environment variable, where there is an environment. */
const env = (name: string): string | undefined => (globalThis as MaybeProcess).process?.env?.[name];

/** A value and where it came from. */
interface Sourced {
  /** The value. */
  value?: string;
  /** Its source, as messages name it. */
  from: string;
}

/** The first source that has the value: the option, then the environment, then the saved config. */
function firstOf(option: string | undefined, name: string, saved: string | undefined, optionName: string): Sourced {
  if (option) return { value: option, from: `the ${optionName} option` };
  if (env(name)) return { value: env(name), from: name };
  return { value: saved, from: '~/.oya/config.json' };
}

/** The address to call and where it came from; the hosted default when nothing names one. */
function addressOf(options: OyaOptions, saved: string | undefined): Sourced {
  const found = firstOf(options.baseUrl, 'OYA_BASE_URL', saved, 'baseUrl');
  return found.value ? found : { value: DEFAULT_BASE_URL, from: 'default' };
}

/** The key, trimmed of the line break a key file ends with, and refused if fetch could not send it. */
function keyOf(options: OyaOptions, saved: string | undefined): Sourced {
  const found = firstOf(options.apiKey, 'OYA_API_KEY', saved, 'apiKey');
  if (!found.value) throw new Error('No API key. Pass { apiKey }, set OYA_API_KEY, or run `npx @oya-ai/cli login`.');
  return { ...found, value: headerSafe(found.value, 'apiKey', 'Copy the key again.') };
}

/** The Http for these options: key and URL from the options, then the environment, then the saved config. */
export function createHttp(options: OyaOptions): Http {
  const saved = savedConfig();
  const key = keyOf(options, saved.apiKey);
  const address = addressOf(options, saved.baseUrl);
  const fetchImpl = options.fetch || globalThis.fetch;
  if (!fetchImpl) throw new Error('No fetch available, pass { fetch } or use Node 18+.');
  const origin = { apiKeyFrom: key.from, baseUrlFrom: address.from, savedKey: !!saved.apiKey };
  const timeout = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  return new Http(webAddress(address), key.value!, timeout, fetchImpl.bind(globalThis), origin);
}

/** A local address, which is served over plain http. */
const LOCAL_HOST = /^(localhost|127\.)/i;

/** The address without a trailing slash, refused before any call when it has no http(s) scheme. */
function webAddress({ value, from }: Sourced): string {
  const url = value!.trim().replace(/\/+$/, '');
  if (/^https?:\/\//i.test(url)) return url;
  const name = from.startsWith('the ') ? 'baseUrl' : from === 'OYA_BASE_URL' ? from : `baseUrl in ${from}`;
  const suggestion = suggestionFor(url);
  const tryIt = suggestion ? ` Try ${suggestion}.` : '';
  throw refusal(`${name} must start with http:// or https://, not "${url}".${tryIt}`, 'baseUrl', suggestion);
}

/** The address with a scheme added, or none when it already names another scheme (ftp://, http:/). */
function suggestionFor(url: string): string | undefined {
  // host:port reads like a scheme to a pattern, but its part after the colon is a number.
  if (/^[a-z][a-z0-9+.-]*:/i.test(url) && !/^[^:/]+:\d+(\/|$)/.test(url)) return undefined;
  return `${LOCAL_HOST.test(url) ? 'http' : 'https'}://${url}`;
}

/** Whether fetch refuses `ch` in a header value, as undici does: a control character other than tab, DEL, or anything above U+00FF. */
function notInAHeader(ch: string): boolean {
  const code = ch.codePointAt(0)!;
  return (code < SPACE && code !== TAB) || code === DEL || code > LATIN1_MAX;
}

/**
 * `value` as a header can carry it: trimmed of spaces and line breaks at the
 * ends (fetch strips those itself), and refused with its position when a
 * character inside it would make fetch throw on every call. Never echoes it.
 */
export function headerSafe(value: string, field: string, ending: string): string {
  const trimmed = value.trim();
  const chars = Array.from(trimmed);
  const at = chars.findIndex(notInAHeader);
  if (at < 0) return trimmed;
  const leading = Array.from(value).length - Array.from(value.trimStart()).length;
  const where = `position ${leading + at + 1} (${codePoint(chars[at])})`;
  throw refusal(`${field} has a character HTTP headers cannot carry at ${where}. ${ending}`, field);
}

/** A character as U+ and its code point, which names it without showing the value around it. */
const codePoint = (ch: string) =>
  `U+${ch.codePointAt(0)!.toString(HEX).toUpperCase().padStart(CODE_POINT_DIGITS, '0')}`;

/** How a refused value is shown: quoted when it is a string, as it is otherwise. */
const shown = (value: unknown) => (typeof value === 'string' ? JSON.stringify(value) : String(value));

/**
 * One path segment for an id, encoded. Refused before any call when it is
 * empty or not a string, or when it would change the path: "/" moves into
 * another resource, and "." and ".." survive encoding and walk up it.
 */
export function segment(value: unknown, field = 'id'): string {
  if (typeof value !== 'string' || !value)
    throw refusal(`${field} must be a non-empty string, not ${shown(value)}`, field);
  if (value.includes('/') || value === '.' || value === '..')
    throw refusal(`${field} must be a single path segment (no "/" and not "." or ".."), not ${shown(value)}`, field);
  return encodeURIComponent(value);
}
