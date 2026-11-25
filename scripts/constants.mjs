/** Shared IndexNow limits keep manual submissions and CI bounded in the same way. */

/** Each request owns a deadline that includes consuming or cancelling its response body. */
export const INDEXNOW_REQUEST_TIMEOUT_MS = 15_000;

/** Converts request deadlines to seconds in actionable command-line errors. */
export const MILLISECONDS_PER_SECOND = 1_000;

/** Keeps the existing dry-run JSON readable for manual inspection. */
export const INDEXNOW_JSON_INDENT = 2;

/** A failed notification returns a nonzero exit code without terminating pending cleanup. */
export const INDEXNOW_FAILURE_EXIT_CODE = 1;
