/**
 * The project API client every dashboard request goes through. A failure
 * always carries the server's own reason, and a refused credential tells the
 * project switcher to renew it.
 */
import { apiUrl, authHeaders } from './api';
import { Status } from './http-status';

export { ago, shortId } from './format';

/** Statuses that mean the credential itself was refused, not just this request. */
const CREDENTIAL_GONE: number[] = [Status.UNAUTHORIZED, Status.FORBIDDEN, Status.GONE];

/** The server's error body. */
interface ErrorBody {
  /** Why the request failed, in the server's words. */
  error?: string;
}

/** A request the server answered with an error status. */
export class ApiError extends Error {
  /** The HTTP status the server answered with. */
  status: number;
  /** The parsed response body (JSON, text, or null when empty). */
  body: unknown;

  /** Carries the server's reason as the message, plus its status and body. */
  constructor(message: string, status: number, body: unknown) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.body = body;
  }
}

/** One request's options. */
export interface ApiOptions {
  /** The project credential to send as the bearer token. */
  key: string;
  /** HTTP method; GET when omitted. */
  method?: string;
  /** Sent as JSON when present. */
  body?: unknown;
  /** Aborts the request. */
  signal?: AbortSignal;
}

/**
 * Every dashboard request goes through here, so a failure always carries the
 * server's own reason — "Persona is in use", not "Request failed (409)".
 */
export async function api<T = unknown>(path: string, { key, method = 'GET', body, signal }: ApiOptions): Promise<T> {
  const json = body === undefined ? undefined : JSON.stringify(body);
  const res = await fetch(apiUrl(path), { method, headers: authHeaders(key), body: json, signal });
  const payload = parsePayload(await res.text());
  if (!res.ok) throw refusal(res.status, payload, `${method} ${path}`);
  return payload as T;
}

/** JSON when the body is JSON, the raw text when it is not, null when empty. */
function parsePayload(text: string): unknown {
  try {
    return text ? JSON.parse(text) : null;
  } catch {
    return text;
  }
}

/** The error for a failed response, announcing a refused credential on the way. */
function refusal(status: number, payload: unknown, request: string): ApiError {
  const reason = (payload as ErrorBody | null)?.error || `${request} failed (${status})`;
  // The credential itself was refused: the project switcher reopens or replaces it.
  if (CREDENTIAL_GONE.includes(status)) window.dispatchEvent(new CustomEvent('oya:credential-gone'));
  return new ApiError(reason, status, payload);
}

/** The message of an error, or `fallback` for anything thrown that is not an Error. */
export const errorMessage = (err: unknown, fallback = 'Something went wrong'): string =>
  err instanceof Error ? err.message : fallback;
