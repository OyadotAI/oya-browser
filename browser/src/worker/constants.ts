/**
 * Every number the validation worker uses, by name: auto-heal limits, how long
 * a step waits for the page to settle, failure message length, and the
 * generated module's file mode.
 */

/** Replay worker: auto-heal attempts per step and the window they must finish in. */
export const REPLAY = {
  MAX_REPAIRS: 2,
  REPAIR_WINDOW_MS: 30000,
  MIN_WAIT_MS: 1,
  /** Longest a step waits for the page's network to go quiet before acting. */
  SETTLE_MS: 5000,
  /** After a step that sent input: the moment a page takes to start what it triggers (a grid's reload). */
  SETTLE_GRACE_MS: 400,
} as const;

/** Failure messages: the longest one a run reports. */
export const FAILURE = { MESSAGE_MAX_CHARS: 300 } as const;

/** The generated module is written owner-only. */
export const MODULE_FILE_MODE = 0o600;
