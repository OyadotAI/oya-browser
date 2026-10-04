/**
 * Every number the workflow studio's main-process side uses, by name: draft
 * encryption, the target picker, validation runs and the workspace.
 */

/** Encrypted draft files: AES-256-GCM key, IV and tag sizes, and owner-only modes. */
export const CRYPTO = { KEY_BYTES: 32, IV_BYTES: 12, TAG_BYTES: 16, DIR_MODE: 0o700, FILE_MODE: 0o600 } as const;

/** Target picker: how long a person has to pick, and the highlight colour (teal). */
export const PICKER = {
  TIMEOUT_MS: 60000,
  HIGHLIGHT: { r: 70, g: 180, b: 160 },
  CONTENT_ALPHA: 0.3,
} as const;

/** Validation: run token size, how long to wait for the debugging port, and the stop grace. */
export const VALIDATION = {
  TOKEN_BYTES: 32,
  PORT_POLLS: 50,
  PORT_POLL_MS: 100,
  STOP_GRACE_MS: 5000,
  /** Longest a validation tab may take to open and report its target before the run fails. */
  TAB_OPEN_MS: 15000,
  /** The longest pause a slowed-down run may take before each step. */
  MAX_SLOW_MO_MS: 10000,
} as const;

/** Workspace: undo depth, run history retention, and how often run progress is published and saved. */
export const WORKSPACE = {
  MAX_HISTORY: 100,
  MAX_RUN_EVENTS: 2000,
  MAX_RUNS: 30,
  /** A week. */
  RUN_MAX_AGE_MS: 604800000,
  /** 250 MiB. */
  RUN_STORAGE_BYTES: 262144000,
  NOTIFY_MS: 100,
  SAVE_RUN_MS: 1000,
  REPAIR_NAME_LENGTH: 45,
  SUPPORT_SCHEMA: 1,
} as const;
