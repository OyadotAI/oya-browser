/**
 * Every number the page commands run on, by name: waits, retries, the pauses
 * that keep input human-like, and fallbacks for a page that reports no size.
 * A range is `{ base, spread }`: `base + Math.random() * spread` milliseconds.
 */

/**
 * Longest a command waits for a tab's first load. A page that never finished
 * used to block every later command on that tab with no result and no error.
 */
const TAB_READY_TIMEOUT_MS = 20000;
/** Longest a command waits for a navigation it caused to finish loading. */
const LOAD_TIMEOUT_MS = 30000;
/** Extra attempts at a navigation that failed for a reason other than being aborted. */
const NAVIGATE_RETRIES = 2;
/** Pause between navigation attempts. */
const NAVIGATE_RETRY_MS = 1000;
/** Time for a click or Enter to start a navigation before checking for one. */
const NAVIGATION_START_MS = 300;
/** Settle time after a coordinate click, hover or double click. */
const AFTER_POINTER_MS = 100;
/** Time for autocomplete suggestions to appear after typing. */
const SUGGESTIONS_MS = 800;
/** Pause after focusing a field from the dev panel, before clearing it. */
const DEV_FOCUS_MS = 20;

/** Pause between clicking a field and typing into it; also how long a key is held. */
const FOCUS_PAUSE = { base: 20, spread: 30 };
/** Pauses around deleting a field's selected content. */
const CLEAR_PAUSE = { base: 10, spread: 15 };
/** Time a masked field (__/__/____) gets to redraw itself after being cleared, before the first key. */
const CLEAR_SETTLE_MS = 150;
/** Pause after reaching a point, before pressing the button. */
const BEFORE_PRESS = { base: 10, spread: 15 };
/** The click count CDP gives the second click of a double click. */
const DOUBLE_CLICK = 2;
/** Gap between the two clicks of a double click. */
const DOUBLE_CLICK_GAP = { base: 60, spread: 40 };
/** Gap between smooth-scroll increments. */
const SCROLL_PAUSE = { base: 30, spread: 30 };

/** Hold after pressing the button before a drag starts moving. */
const DRAG_PRESS_MS = 30;
/** Points a drag passes through. */
const DRAG_STEPS = 10;
/** Gap between drag points. */
const DRAG_STEP_MS = 10;

/** Viewport assumed when the page does not report one. */
const FALLBACK_VIEWPORT = { w: 800, h: 600 };
/** The centre of a length is its half. */
const HALF = 2;
/** Settle time after a smooth scroll, before analysing the page. */
const SCROLL_SETTLE_MS = 300;
/** Default scroll for a server command, in pixels. */
const SCROLL_AMOUNT = 500;
/** Default scroll for the dev panel's buttons, in pixels. */
const DEV_SCROLL_AMOUNT = 400;
/** Pixels per smooth-scroll increment (one wheel notch). */
const SCROLL_STEP_PX = 120;
/** Fewest increments in a smooth scroll. */
const MIN_SCROLL_STEPS = 3;
/** Default wait for an element to appear. */
const ELEMENT_WAIT_MS = 10000;
/** JPEG quality of a screenshot taken for a model: a retina PNG is megabytes. */
const SCREENSHOT_JPEG_QUALITY = 70;

module.exports = {
  SCREENSHOT_JPEG_QUALITY,
  TAB_READY_TIMEOUT_MS,
  LOAD_TIMEOUT_MS,
  NAVIGATE_RETRIES,
  NAVIGATE_RETRY_MS,
  NAVIGATION_START_MS,
  AFTER_POINTER_MS,
  SUGGESTIONS_MS,
  DEV_FOCUS_MS,
  FOCUS_PAUSE,
  CLEAR_PAUSE,
  CLEAR_SETTLE_MS,
  BEFORE_PRESS,
  DOUBLE_CLICK,
  DOUBLE_CLICK_GAP,
  SCROLL_PAUSE,
  DRAG_PRESS_MS,
  DRAG_STEPS,
  DRAG_STEP_MS,
  FALLBACK_VIEWPORT,
  HALF,
  SCROLL_SETTLE_MS,
  SCROLL_AMOUNT,
  DEV_SCROLL_AMOUNT,
  SCROLL_STEP_PX,
  MIN_SCROLL_STEPS,
  ELEMENT_WAIT_MS,
};
