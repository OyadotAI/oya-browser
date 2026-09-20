/**
 * HTTP status codes by name, so a check reads as what it means
 * (`Status.UNAUTHORIZED`) rather than a bare number.
 */

/** The statuses the console reacts to. */
export const Status = {
  /** The credential is missing or was not accepted. */
  UNAUTHORIZED: 401,
  /** The credential is valid but may not do this. */
  FORBIDDEN: 403,
  /** The credential existed once and has been revoked or expired. */
  GONE: 410,
} as const;
