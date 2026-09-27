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
  /** Nothing lives at this address. */
  NOT_FOUND: 404,
  /** What was asked for clashes with something already there, such as a name. */
  CONFLICT: 409,
  /** The credential existed once and has been revoked or expired. */
  GONE: 410,
  /** Something this server depends on answered badly or not at all. */
  BAD_GATEWAY: 502,
} as const;
