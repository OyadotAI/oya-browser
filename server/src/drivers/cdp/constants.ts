/**
 * Every number the CDP driver runs on, by name: timeouts, limits and the
 * shapes of the synthetic input it sends.
 */

/** How long opening the CDP socket may take. */
export const CONNECT_TIMEOUT_MS = 15_000;
/** Largest CDP message accepted (256 MiB): full-page screenshots are big. */
export const MAX_PAYLOAD_BYTES = 268_435_456;
/** Default wait for one CDP reply, and for one driver command. */
export const COMMAND_TIMEOUT_MS = 30_000;
/** A command step never gets less than this, however little of its budget is left. */
export const MIN_STEP_MS = 1000;
/** Longest wait for a page's load event after navigating. */
export const LOAD_WAIT_MAX_MS = 60_000;

/** Recorded steps kept per recording. */
export const MAX_RECORDED_STEPS = 500;
/** Random bytes in the per-session analyzer tag attribute. */
export const TAG_BYTES = 4;
/** Random bytes in the isolated world's name. */
export const WORLD_NAME_BYTES = 8;

/** JPEG quality of an on-demand screenshot. */
export const SCREENSHOT_QUALITY = 60;
/** Viewport assumed when the page cannot report its own. */
export const FALLBACK_VIEWPORT = { width: 1280, height: 800 };
/** The second click of a double click. */
export const DOUBLE_CLICK = 2;
/** Intermediate pointer moves in a drag. */
export const DRAG_STEPS = 4;
/** A scroll without an amount moves this fraction of the viewport. */
export const SCROLL_PAGE_FRACTION = 0.8;
/** Where the wheel event lands when the caller gives no point. */
export const WHEEL_ORIGIN = 10;
/** Default wait for an element to appear. */
export const WAIT_TIMEOUT_MS = 10_000;
/** How often a wait looks for the element. */
export const WAIT_POLL_MS = 250;

/** Live-view JPEG quality. */
export const SCREENCAST_QUALITY = 40;
/** Live-view frame width cap. */
export const SCREENCAST_MAX_WIDTH = 1280;
/** Chrome sends every Nth painted frame. */
export const SCREENCAST_EVERY_NTH_FRAME = 2;
/** How often the idle fill checks whether the live view has gone quiet. */
export const IDLE_FILL_INTERVAL_MS = 1000;
/** Quiet this long and the idle fill sends a screenshot. */
export const IDLE_FRAME_GAP_MS = 1200;
/** Timeout of one idle-fill screenshot. */
export const IDLE_FILL_TIMEOUT_MS = 5000;
