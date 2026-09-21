/**
 * The one HTTP path. Everything else in this package is a wrapper over it: it
 * adds the bearer key, encodes the body, applies the timeout and turns a
 * failed answer into an OyaError. `createHttp` builds it from the client's
 * options and the environment.
 */
import { OyaError } from './errors.js';
import { savedConfig } from './cli-config.js';
import { DEFAULT_BASE_URL, DEFAULT_TIMEOUT_MS } from './constants.js';
import type { OyaOptions } from './types/index.js';

/** The one HTTP path. Everything else in this package is a wrapper over it. */
export class Http {
  /** Stores where to call, with which key, how long to wait and which fetch to use. */
  constructor(
    readonly baseUrl: string,
    readonly apiKey: string,
    private readonly timeoutMs: number,
    private readonly fetchImpl: typeof globalThis.fetch,
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
    return readAnswer<T>(res, `${method} ${path}`, this.baseUrl);
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
      throw new OyaError(unreachable(this.baseUrl, timeoutMs, err), 0, { error: String((err as Error)?.message) });
    }
  }
}

/** Why the request never got an answer, in words that name the address. */
function unreachable(baseUrl: string, timeoutMs: number, err: unknown): string {
  const cause = (err as Error)?.name === 'TimeoutError' || /abort/i.test(String((err as Error)?.message));
  if (cause) return `No answer from ${baseUrl} within ${timeoutMs}ms. Is it reachable, and is the call this slow?`;
  return `Could not reach ${baseUrl}. Is the server running, and is OYA_BASE_URL right?`;
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
async function readAnswer<T>(res: Response, call: string, baseUrl: string): Promise<T> {
  const payload = parseBody(await res.text());
  if (!res.ok) throw failure(call, res.status, payload, baseUrl);
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
function failure(call: string, status: number, payload: unknown, baseUrl: string): OyaError {
  let message = (payload as ErrorBody)?.error || `${call} failed (${status})`;
  if (message === 'Invalid API key') {
    message += ` for ${baseUrl}. Check OYA_API_KEY: a value exported in your shell beats .env.`;
  }
  return new OyaError(message, status, payload);
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

/** The Http for these options: key and URL from the options, then the environment, then defaults. */
export function createHttp(options: OyaOptions): Http {
  const saved = savedConfig();
  const apiKey = options.apiKey || env('OYA_API_KEY') || saved.apiKey;
  if (!apiKey) throw new Error('No API key. Pass { apiKey }, set OYA_API_KEY, or run `npx @oya-ai/cli login`.');
  const baseUrl = (options.baseUrl || env('OYA_BASE_URL') || saved.baseUrl || DEFAULT_BASE_URL).replace(/\/+$/, '');
  const fetchImpl = options.fetch || globalThis.fetch;
  if (!fetchImpl) throw new Error('No fetch available, pass { fetch } or use Node 18+.');
  return new Http(baseUrl, apiKey, options.timeoutMs ?? DEFAULT_TIMEOUT_MS, fetchImpl.bind(globalThis));
}
