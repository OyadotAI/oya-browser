/**
 * Errors that carry their HTTP answer. A route or service throws one, and the
 * API's error handler (app/api.ts) turns it into the response, so no handler
 * needs its own try/catch just to translate an error.
 */

/** An error with the status (and optional code or detail) it should be answered with. */
export class HttpError extends Error {
  /** HTTP status to answer with. */
  declare status: number;
  /** Machine-readable reason, when a caller branches on it. */
  declare code?: string;

  /** `extra` carries any further fields callers read (code, active, retryAfter…). */
  constructor(status: number, message: string, extra: Record<string, unknown> = {}) {
    super(message);
    this.status = status;
    Object.assign(this, extra);
  }
}
