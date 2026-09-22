/**
 * Numbers the control socket (main/connection/) runs on: close codes, the
 * heartbeat, reconnect backoff, and limits on what is logged.
 */

/** WebSocket close codes shared with the server (server/src/modules/browsers/connection/constants.ts). */
const CloseCode = {
  /** Same browser reconnected; this socket is the old one. */
  REPLACED: 4000,
  /** No auth message in time. */
  AUTH_TIMEOUT: 4001,
  /** Bad credential, someone else's browser id, or a session that could not be set up: do not retry. */
  REJECTED: 4003,
};

/** How often this browser pings the server. */
const PING_INTERVAL_MS = 20000;
/** Pings without an answer tolerated before the socket is closed. */
const MAX_MISSED_PONGS = 4;
/** First reconnect delay, before backoff. */
const RECONNECT_BASE_MS = 500;
/** Each failed reconnect waits this much longer than the last. */
const RECONNECT_GROWTH = 1.5;
/** The longest reconnect delay, before jitter. */
const RECONNECT_MAX_MS = 10000;
/** Random extra delay, so a fleet does not reconnect in lockstep. */
const RECONNECT_JITTER_MS = 500;

/** Live-view frame rate when the server names none. */
const DEFAULT_STREAM_FPS = 2;
/** Characters of a command id shown in the dev log. */
const ID_PREVIEW_CHARS = 8;
/** Characters of an analysis's page (markdown, TOON) shown in the dev log. */
const MARKDOWN_PREVIEW_CHARS = 500;
/** Characters of an unparseable server answer shown to the person. */
const ERROR_PREVIEW_CHARS = 200;
/** Bytes per kilobyte, for screenshot sizes in the log. */
const BYTES_PER_KB = 1024;
/** Random bytes in a browser id. */
const BROWSER_ID_BYTES = 8;
/** Hex digits per byte. */
const HEX_BYTE_DIGITS = 2;
/** Hex. */
const HEX = 16;

/** The longest a remote workflow run may take. */
const WORKFLOW_LIMIT_MS = 540000;
/** How often a remote workflow run is checked on. */
const WORKFLOW_POLL_MS = 200;
/** Longest a playbook save may wait for the server before the panel says so. */
const SAVE_TIMEOUT_MS = 20000;

/**
 * Codes a command result may carry to the server, which answers with them.
 * Only ours: an Electron ERR_* or a Node E* code on a thrown error names this
 * machine's internals, not a reason a caller can act on.
 */
const RESULT_CODES = new Set(['tab_unprotected', 'action_unsupported']);

module.exports = {
  CloseCode,
  RESULT_CODES,
  PING_INTERVAL_MS,
  MAX_MISSED_PONGS,
  RECONNECT_BASE_MS,
  RECONNECT_GROWTH,
  RECONNECT_MAX_MS,
  RECONNECT_JITTER_MS,
  DEFAULT_STREAM_FPS,
  ID_PREVIEW_CHARS,
  MARKDOWN_PREVIEW_CHARS,
  ERROR_PREVIEW_CHARS,
  BYTES_PER_KB,
  BROWSER_ID_BYTES,
  HEX_BYTE_DIGITS,
  HEX,
  WORKFLOW_LIMIT_MS,
  WORKFLOW_POLL_MS,
  SAVE_TIMEOUT_MS,
};
