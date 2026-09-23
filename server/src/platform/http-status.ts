/**
 * HTTP status codes by name, so a handler reads `Status.NOT_FOUND` instead of
 * a bare 404.
 */

/** The statuses this server answers with. */
export const Status = {
  OK: 200,
  CREATED: 201,
  ACCEPTED: 202,
  NO_CONTENT: 204,
  /** A range of a file, as a resumable download asks for. */
  PARTIAL_CONTENT: 206,
  FOUND: 302,
  PERMANENT_REDIRECT: 308,
  BAD_REQUEST: 400,
  UNAUTHORIZED: 401,
  FORBIDDEN: 403,
  NOT_FOUND: 404,
  CONFLICT: 409,
  GONE: 410,
  PAYLOAD_TOO_LARGE: 413,
  UNPROCESSABLE: 422,
  TOO_MANY_REQUESTS: 429,
  REQUEST_TIMEOUT: 408,
  INTERNAL: 500,
  NOT_IMPLEMENTED: 501,
  BAD_GATEWAY: 502,
  UNAVAILABLE: 503,
  GATEWAY_TIMEOUT: 504,
  /** Anthropic's "overloaded": not a standard status, but one its API answers with. */
  OVERLOADED: 529,
} as const;
