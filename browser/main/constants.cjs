/**
 * Every number the main-process helper modules (main/*.cjs) run on, by name:
 * timings, limits and sizes. One place to read what the browser waits for and
 * how much it accepts.
 */

/** Bytes in a mebibyte. */
const MIB = 1_048_576;

/** Largest CDP message the relay and the front door accept (256 MiB): a full-page screenshot is big. */
const MAX_CDP_PAYLOAD = 268_435_456;

/** A host's cookies are pulled from the pool at most this often. */
const COOKIE_PULL_TTL_MS = 30_000;
/** A navigation never waits longer than this for a cookie pull. */
const COOKIE_PULL_TIMEOUT_MS = 3000;
/** Local cookie changes are batched for this long before they are sent. */
const COOKIE_FLUSH_MS = 2000;
/** A batch this large is sent at once rather than waiting out the timer. */
const COOKIE_BATCH_MAX = 200;
/** Hosts remembered as recently pulled; past this the memory is cleared. */
const COOKIE_PULLED_HOSTS_MAX = 500;

/** The fastest the live view streams: one frame per this many ms. */
const STREAM_MIN_FRAME_MS = 200;
/** Milliseconds per second, to turn frames per second into an interval. */
const MS_PER_SECOND = 1000;
/** Frames are skipped while the socket has this much still unsent. */
const STREAM_MAX_BUFFERED = MIB;
/** JPEG quality of live-view frames. */
const STREAM_JPEG_QUALITY = 40;

/** How long a pairing claim may take before it is abandoned. */
const PAIRING_TIMEOUT_MS = 15_000;

/** How often a packaged app checks for an update (six hours). */
const UPDATE_CHECK_INTERVAL_MS = 21_600_000;
/** The first check waits this long, so the first window can settle. */
const UPDATE_FIRST_CHECK_MS = 10_000;

/** How often the routines scheduler looks for a routine that is due (one minute, its finest schedule). */
const ROUTINE_TICK_MS = 60_000;
/** Milliseconds in each unit an "every N" routine may repeat in. */
const ROUTINE_UNIT_MS = { minutes: 60_000, hours: 3_600_000 };
/** How much of a run's answer a routine's history keeps. */
const ROUTINE_RESULT_CHARS = 4000;
/** How many of a run's steps (tool names) its history keeps. */
const ROUTINE_STEPS_KEPT = 100;

/** Chrome version assumed when the session's user agent names none. */
const FALLBACK_CHROME_VERSION = '134.0.0.0';
/** Random bytes in an isolated world's per-document tag attribute. */
const WORLD_ATTR_BYTES = 4;

/**
 * Listeners one tab's debugger may carry before Node warns. A tab keeps about 8
 * at rest (persona, workers, dialogs, login state) and 12 while recording, all
 * removed when their owner stops; the default warning at 10 cried leak at every
 * recording. Measured flat across record/stop cycles; a real leak still trips 50.
 */
const DEBUGGER_MAX_LISTENERS = 50;

module.exports = {
  MAX_CDP_PAYLOAD,
  COOKIE_PULL_TTL_MS,
  COOKIE_PULL_TIMEOUT_MS,
  COOKIE_FLUSH_MS,
  COOKIE_BATCH_MAX,
  COOKIE_PULLED_HOSTS_MAX,
  STREAM_MIN_FRAME_MS,
  MS_PER_SECOND,
  STREAM_MAX_BUFFERED,
  STREAM_JPEG_QUALITY,
  PAIRING_TIMEOUT_MS,
  UPDATE_CHECK_INTERVAL_MS,
  UPDATE_FIRST_CHECK_MS,
  ROUTINE_TICK_MS,
  ROUTINE_UNIT_MS,
  ROUTINE_RESULT_CHARS,
  ROUTINE_STEPS_KEPT,
  FALLBACK_CHROME_VERSION,
  WORLD_ATTR_BYTES,
  DEBUGGER_MAX_LISTENERS,
};
