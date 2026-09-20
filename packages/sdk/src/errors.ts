/**
 * OyaError: the one error type the SDK throws for a failed call, carrying the
 * HTTP status and the server's answer so a caller can branch on either.
 */

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
