/**
 * Every number the SDK runs on, by name: time budgets for each kind of call,
 * polling cadence, file limits and the HTTP statuses it reports errors with.
 */

/** The hosted control plane, used when neither `baseUrl` nor OYA_BASE_URL is set. */
export const DEFAULT_BASE_URL = 'https://browser.getoya.ai';

/** Per-request timeout when the caller does not pass `timeoutMs`. */
export const DEFAULT_TIMEOUT_MS = 60_000;
/** Starting or stopping browsers can wait on a sandbox, so it gets two minutes. */
export const START_TIMEOUT_MS = 120_000;
/** How long `start()` waits for a cloud browser to dial in, before any queueing time. */
export const READY_TIMEOUT_MS = 120_000;
/** How often `start()` checks whether a cloud browser has connected. */
export const READY_POLL_MS = 2_000;

/** Navigation is slow and the server disables its own timeout for it. */
export const NAVIGATE_TIMEOUT_MS = 120_000;
/** CAPTCHA solving and MFA can wait on a third-party solver or a mailbox. */
export const CHALLENGE_TIMEOUT_MS = 180_000;
/** An agent run (`ask`, `play`) can take many steps. */
export const AGENT_TIMEOUT_MS = 600_000;
/** Saving a playbook compiles the last run into steps and code. */
export const PLAYBOOK_TIMEOUT_MS = 120_000;
/** Stopping one browser: a cloud sandbox is destroyed before this answers. */
export const STOP_TIMEOUT_MS = 60_000;
/** How long `waitFor()` waits for a selector by default. */
export const WAIT_FOR_DEFAULT_MS = 30_000;
/** Extra request time on top of `waitFor()`'s own timeout, so the server answers first. */
export const WAIT_FOR_GRACE_MS = 5_000;
/** Pixels an aimed scroll moves when no amount is given. */
export const AIMED_SCROLL_AMOUNT = 500;

/** How often a submitted run is polled by default. */
export const RUN_POLL_MS = 2_000;
/** Consecutive failed polls before a run is reported as failed. */
export const MAX_RUN_POLL_ERRORS = 5;

/** Bytes in a megabyte, for file limits and messages. */
export const BYTES_PER_MB = 1_048_576;
/** Largest task file, in megabytes: bigger and the server would refuse the body anyway. */
export const MAX_FILE_MB = 10;
/** Bytes turned into a string per `String.fromCharCode` call; spreading more blows the stack. */
export const BASE64_CHUNK_BYTES = 8192;

/** Milliseconds per second, for human-readable timeouts. */
export const MS_PER_SECOND = 1000;

/** HTTP statuses the SDK reports its own errors with, or reads from the server's, by name. */
export const Status = {
  /** The caller passed something unusable, such as a non-numeric element id. */
  BAD_REQUEST: 400,
  /** The server does not know that session. */
  NOT_FOUND: 404,
  /** The browser is in a state that needs attention first. */
  CONFLICT: 409,
  /** A browser command ran and failed. */
  UNPROCESSABLE: 422,
  /** A run failed without saying why. */
  SERVER_ERROR: 500,
  /** A browser did not come up in time. */
  GATEWAY_TIMEOUT: 504,
} as const;
