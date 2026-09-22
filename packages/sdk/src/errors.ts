/**
 * OyaError: the one error type the SDK throws for a failed call, carrying the
 * HTTP status and the server's answer so a caller can branch on either.
 */
import { Status } from './constants.js';

/** A failed API call or browser command. */
export class OyaError extends Error {
  /** The HTTP status, or the SDK's own status for errors it raises itself. */
  readonly status: number;
  /** The server's response body, parsed when it was JSON. */
  readonly body: unknown;
  /** Records the message, status and body. */
  constructor(message: string, status: number, body: unknown) {
    super(message);
    this.name = 'OyaError';
    this.status = status;
    this.body = body;
  }
}

/** The body of a request refused before it was sent, in the server's own shape, so `body.field` reads the same either way. */
export interface Refusal {
  /** What was wrong and what it needed. */
  error: string;
  /** Always invalid_request: the caller's input, nothing the server said. */
  code: 'invalid_request';
  /** The option or argument that was wrong. */
  field: string;
  /** A value that would work, when there is an obvious one. */
  suggestion?: string;
}

/** An OyaError for input refused before any request, status 400 like the server's own. */
export function refusal(message: string, field: string, suggestion?: string): OyaError {
  const body: Refusal = { error: message, code: 'invalid_request', field, ...(suggestion ? { suggestion } : {}) };
  return new OyaError(message, Status.BAD_REQUEST, body);
}
