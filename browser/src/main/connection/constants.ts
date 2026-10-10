/**
 * Numbers the control socket (src/main/connection/) runs on: close codes, the
 * heartbeat, reconnect backoff, limits on what is logged, the live view's
 * pacing, the CDP relay's payload cap and the pairing claim's time limit.
 */

/** WebSocket close codes shared with the server (server/src/modules/browsers/connection/constants.ts). */
export const CloseCode = {
  /** Same browser reconnected; this socket is the old one. */
  REPLACED: 4000,
  /** No auth message in time. */
  AUTH_TIMEOUT: 4001,
  /** Bad credential, someone else's browser id, or a session that could not be set up: do not retry. */
  REJECTED: 4003,
} as const;

/** How often this browser pings the server. */
export const PING_INTERVAL_MS = 20000;
/** Pings without an answer tolerated before the socket is closed. */
export const MAX_MISSED_PONGS = 4;
/** First reconnect delay, before backoff. */
export const RECONNECT_BASE_MS = 500;
/** Each failed reconnect waits this much longer than the last. */
export const RECONNECT_GROWTH = 1.5;
/** The longest reconnect delay, before jitter. */
export const RECONNECT_MAX_MS = 10000;
/** Random extra delay, so a fleet does not reconnect in lockstep. */
export const RECONNECT_JITTER_MS = 500;

/** Live-view frame rate when the server names none. */
export const DEFAULT_STREAM_FPS = 2;
/** Characters of a command id shown in the dev log. */
export const ID_PREVIEW_CHARS = 8;
/** Characters of an analysis's page (markdown, TOON) shown in the dev log. */
export const MARKDOWN_PREVIEW_CHARS = 500;
/** Characters of an unparseable server answer shown to the person. */
export const ERROR_PREVIEW_CHARS = 200;
/** Bytes per kilobyte, for screenshot sizes in the log. */
export const BYTES_PER_KB = 1024;
/** Random bytes in a browser id. */
export const BROWSER_ID_BYTES = 8;
/** Hex digits per byte. */
export const HEX_BYTE_DIGITS = 2;
/** Hex. */
export const HEX = 16;

/** The longest a remote workflow run may take. */
export const WORKFLOW_LIMIT_MS = 540000;
/** How often a remote workflow run is checked on. */
export const WORKFLOW_POLL_MS = 200;
/** Longest a playbook save may wait for the server before the panel says so. */
export const SAVE_TIMEOUT_MS = 20000;

/**
 * Codes a command result may carry to the server, which answers with them.
 * Only ours: an Electron ERR_* or a Node E* code on a thrown error names this
 * machine's internals, not a reason a caller can act on.
 */
export const RESULT_CODES: ReadonlySet<unknown> = new Set(['tab_unprotected', 'action_unsupported']);

/** Bytes in a mebibyte. */
const MIB = 1_048_576;
/** Largest CDP message the relay accepts (256 MiB): a full-page screenshot is big. */
export const MAX_CDP_PAYLOAD = 268_435_456;
/** Bound native resource ownership across remote gateway connections. */
export const MAX_NATIVE_RELAYS = 128;

/** The fastest the live view streams: one frame per this many ms. */
export const STREAM_MIN_FRAME_MS = 200;
/** Milliseconds per second, to turn frames per second into an interval. */
export const MS_PER_SECOND = 1000;
/** Frames are skipped while the socket has this much still unsent. */
export const STREAM_MAX_BUFFERED = MIB;
/** JPEG quality of live-view frames. */
export const STREAM_JPEG_QUALITY = 40;

/** How long a pairing claim may take before it is abandoned. */
export const PAIRING_TIMEOUT_MS = 15_000;

/** Follow background playbook runs without depending on the visible panel. */
export const PLAYBOOK_POLL_MS = 1000;
