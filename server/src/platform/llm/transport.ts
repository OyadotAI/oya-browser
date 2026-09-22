/**
 * What every provider shares: the request timeout, the error contract, and the
 * retry policy. One policy for all of them, so a rate limit from Claude, Gemini or
 * OpenAI is handled the same way, and a provider SDK's own retries stay off.
 */
import {
  LLM_ATTEMPTS,
  LLM_BACKOFF_FACTOR,
  LLM_ERROR_LOG_CHARS,
  LLM_RETRY_BASE_MS,
  LLM_RETRY_MAX_MS,
  LLM_TIMEOUT_MS,
  MS_PER_SECOND,
} from '../constants.ts';
import { Status } from '../http-status.ts';

/** Statuses worth another attempt: timeout, conflict, rate limit, server errors, and Anthropic's overload. */
const RETRY_STATUS: number[] = [
  Status.REQUEST_TIMEOUT,
  Status.CONFLICT,
  Status.TOO_MANY_REQUESTS,
  Status.INTERNAL,
  Status.BAD_GATEWAY,
  Status.UNAVAILABLE,
  Status.GATEWAY_TIMEOUT,
  Status.OVERLOADED,
];

/** A failed model request: its HTTP status (null when there was no answer) and how long the provider asked to wait. */
export class LlmError extends Error {
  /** The HTTP status, or null for a timeout or a dropped connection. */
  readonly status: number | null;
  /** The provider's retry-after, in ms, when it gave one. */
  readonly retryAfterMs: number | null;

  /** `status` null means the request never got an answer. */
  constructor(status: number | null, message: string, retryAfterMs: number | null = null) {
    super(message);
    this.status = status;
    this.retryAfterMs = retryAfterMs;
  }
}

/** A retry-after header (in seconds) as ms, or null when it is absent or not a number. */
export function retryAfter(value: string | null | undefined) {
  const seconds = Number(value);
  return value && Number.isFinite(seconds) ? seconds * MS_PER_SECOND : null;
}

/** A failure with a status, logged with the start of what the provider said, as the error callers see. */
export function statusError(status: number, said: string, retryAfterValue?: string | null) {
  console.error(`[llm] endpoint ${status}: ${said.slice(0, LLM_ERROR_LOG_CHARS)}`);
  return new LlmError(status, `LLM endpoint returned ${status}`, retryAfter(retryAfterValue));
}

/**
 * A non-2xx answer as an LlmError. The body is logged, never returned: the base URL is
 * tenant-configurable, and echoing what the endpoint said would turn a misconfigured
 * (or deliberately pointed) URL into a read primitive for the caller.
 */
export async function refuse(res: Response): Promise<never> {
  throw statusError(res.status, await res.text(), res.headers.get('retry-after'));
}

/** A request that never got an answer (timeout, dropped connection) as an LlmError. */
export function unanswered(err: any) {
  const timedOut = err?.name === 'TimeoutError' || err?.name === 'AbortError';
  return new LlmError(null, timedOut ? 'LLM endpoint did not answer in time' : 'LLM endpoint could not be reached');
}

/** A base URL without trailing slashes: written either way, and the doubled slash is a 404 on Google's OpenAI endpoint. */
export const baseOf = (baseUrl: string) => baseUrl.replace(/\/+$/, '');

/** POSTs JSON with the shared timeout; a 30x is refused, since it could lead into an internal address. */
export async function postJson(url: string, headers: Record<string, string>, body: unknown) {
  const init = { method: 'POST', redirect: 'error' as const, signal: AbortSignal.timeout(LLM_TIMEOUT_MS) };
  const res = await fetch(url, { ...init, headers, body: JSON.stringify(body) }).catch((err) => {
    throw unanswered(err);
  });
  if (!res.ok) await refuse(res);
  return res.json();
}

/** Whether a failure is worth another attempt: no answer, or a status in LLM_RETRY_STATUS. */
export const retryable = (err: unknown) =>
  err instanceof LlmError && (err.status === null || RETRY_STATUS.includes(err.status));

/** The wait before attempt `n` (1-based retry count): the provider's retry-after, else doubling with jitter. */
export function backoff(n: number, err: LlmError, random = Math.random) {
  const doubled = LLM_RETRY_BASE_MS * LLM_BACKOFF_FACTOR ** (n - 1) * (1 + random());
  return Math.min(err.retryAfterMs ?? doubled, LLM_RETRY_MAX_MS);
}

/** Resolves after `ms`. */
const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Runs `attempt` until it succeeds, fails for good, or LLM_ATTEMPTS run out. `wait` is injectable for tests. */
export async function withRetries<T>(attempt: () => Promise<T>, wait = pause): Promise<T> {
  for (let n = 1; ; n++) {
    try {
      return await attempt();
    } catch (err) {
      if (!retryable(err) || n >= LLM_ATTEMPTS) throw err;
      await wait(backoff(n, err as LlmError));
    }
  }
}
