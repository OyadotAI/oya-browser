/**
 * Every number the workflow scripts use, by name: limits on recorded drafts,
 * timeouts, storage sizes and file modes.
 */

/** Diagnostics: how deep, how long and how many entries a redacted value keeps. */
const REDACT = { MAX_DEPTH: 8, MAX_TEXT: 4000, MAX_ITEMS: 100, MIN_SECRET_LENGTH: 2 };

/** Draft limits: the most a normalized draft or step may hold. */
const DRAFT = {
  SCHEMA_VERSION: 2,
  MAX_STEPS: 500,
  MAX_VARIABLES: 100,
  MAX_NAME: 64,
  MAX_DESCRIPTION: 2000,
  MAX_VALUE: 16000,
  MAX_CANDIDATES: 12,
  MAX_CANDIDATE_VALUE: 4000,
  MAX_TAB_NAME: 100,
  MAX_FRAMES: 10,
};

/** Step timing, in milliseconds, and the default scroll distance in pixels. */
const STEP = { MIN_TIMEOUT: 500, DEFAULT_TIMEOUT: 15000, MAX_TIMEOUT: 90000, DEFAULT_SCROLL: 500, MAX_SCROLL: 100000 };

/** Encrypted draft files: AES-256-GCM key, IV and tag sizes, and owner-only modes. */
const CRYPTO = { KEY_BYTES: 32, IV_BYTES: 12, TAG_BYTES: 16, DIR_MODE: 0o700, FILE_MODE: 0o600 };

/** Recording: the binding name's random bytes, how long the page has to connect, and CDP's name/value attribute pairs. */
const RECORDING = { BINDING_BYTES: 12, READY_TIMEOUT_MS: 3000, ATTRIBUTE_STRIDE: 2 };

/** Target picker: how long a person has to pick, and the highlight colour (teal). */
const PICKER = {
  TIMEOUT_MS: 60000,
  HIGHLIGHT: { r: 70, g: 180, b: 160 },
  CONTENT_ALPHA: 0.3,
};

/**
 * Page rendering: the longest page an analysis returns, the budget cut keeps for
 * its note, and the element index's off-screen list and link lengths.
 */
const PAGE_RENDER = { MAX_PAGE_CHARS: 80000, TRUNCATED_ROOM: 120, MAX_OFFSCREEN_LISTED: 30, MAX_INDEX_LINK: 80 };

/** Validation: run token size, how long to wait for the debugging port, and the stop grace. */
const VALIDATION = {
  TOKEN_BYTES: 32,
  PORT_POLLS: 50,
  PORT_POLL_MS: 100,
  STOP_GRACE_MS: 5000,
  /** Longest a validation tab may take to open and report its target before the run fails. */
  TAB_OPEN_MS: 15000,
};

/** Replay worker: auto-heal attempts per step and the window they must finish in. */
const REPLAY = {
  MAX_REPAIRS: 2,
  REPAIR_WINDOW_MS: 30000,
  MIN_WAIT_MS: 1,
  /** Longest a step waits for the page's network to go quiet before acting. */
  SETTLE_MS: 5000,
  /** After a step that sent input: the moment a page takes to start what it triggers (a grid's reload). */
  SETTLE_GRACE_MS: 400,
};

/** Workspace: undo depth, run history retention, and how often run progress is published and saved. */
const WORKSPACE = {
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
};

/**
 * Dates typed into native date inputs: the last month, day, hour and minute,
 * where a two-digit year is placed, the noon hour for AM/PM, and pad widths.
 */
const DATES = {
  MONTHS: 12,
  MAX_DAY: 31,
  HOURS: 24,
  MINUTES: 60,
  CENTURY: 2000,
  NOON: 12,
  YEAR_WIDTH: 4,
  PART_WIDTH: 2,
};

/** The agent's read_elements: how many elements it lists when not told. */
const QUERIES = { DEFAULT_LIMIT: 50 };

module.exports = {
  PAGE_RENDER,
  REDACT,
  DRAFT,
  STEP,
  CRYPTO,
  RECORDING,
  PICKER,
  VALIDATION,
  REPLAY,
  WORKSPACE,
  DATES,
  QUERIES,
};
