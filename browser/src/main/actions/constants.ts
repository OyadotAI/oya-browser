/**
 * Every number the page commands run on, by name: waits, retries, the pauses
 * that keep input human-like, and fallbacks for a page that reports no size.
 * A range is `{ base, spread }`: `base + Math.random() * spread` milliseconds.
 */
import type { Range } from '../input/constants.ts';

/**
 * Longest a command waits for a tab's first load. A page that never finished
 * used to block every later command on that tab with no result and no error.
 */
export const TAB_READY_TIMEOUT_MS = 20000;
/** Longest a command waits for a navigation it caused to finish loading. */
export const LOAD_TIMEOUT_MS = 30000;
/** Extra attempts at a navigation that failed for a reason other than being aborted. */
export const NAVIGATE_RETRIES = 2;
/** Pause between navigation attempts. */
export const NAVIGATE_RETRY_MS = 1000;
/** Time for a click or Enter to start a navigation before checking for one. */
export const NAVIGATION_START_MS = 300;
/** Settle time after a coordinate click, hover or double click. */
export const AFTER_POINTER_MS = 100;
/**
 * Pause before analysing a page again that read as having no elements at all.
 * A heavy app (a payer portal, a store) can answer a navigation before it has
 * drawn anything, and an empty read tells the agent the page is broken.
 */
export const EMPTY_ANALYSIS_RETRY_MS = 1200;
/** Time for autocomplete suggestions to appear after typing. */
export const SUGGESTIONS_MS = 800;
/** Pause after focusing a field from the dev panel, before clearing it. */
export const DEV_FOCUS_MS = 20;

/** Pause between clicking a field and typing into it; also how long a key is held. */
export const FOCUS_PAUSE: Range = { base: 20, spread: 30 };
/** Pauses around deleting a field's selected content. */
export const CLEAR_PAUSE: Range = { base: 10, spread: 15 };
/** Time a masked field (__/__/____) gets to redraw itself after being cleared, before the first key. */
export const CLEAR_SETTLE_MS = 150;
/** Pause after reaching a point, before pressing the button. */
export const BEFORE_PRESS: Range = { base: 10, spread: 15 };
/** The click count CDP gives the second click of a double click. */
export const DOUBLE_CLICK = 2;
/** Gap between the two clicks of a double click. */
export const DOUBLE_CLICK_GAP: Range = { base: 60, spread: 40 };
/** Gap between smooth-scroll increments. */
export const SCROLL_PAUSE: Range = { base: 30, spread: 30 };

/** Viewport assumed when the page does not report one. */
export const FALLBACK_VIEWPORT = { w: 800, h: 600 } as const;
/** The centre of a length is its half. */
export const HALF = 2;
/** Settle time after a smooth scroll, before analysing the page. */
export const SCROLL_SETTLE_MS = 300;
/** Default scroll for a server command, in pixels. */
export const SCROLL_AMOUNT = 500;
/** The most one scroll command moves, in pixels: the server's cap for a recorded scroll. Past it a scroll would run for minutes. */
export const MAX_SCROLL_AMOUNT = 100_000;
/** Default scroll for the dev panel's buttons, in pixels. */
export const DEV_SCROLL_AMOUNT = 400;
/** Pixels per smooth-scroll increment (one wheel notch). */
export const SCROLL_STEP_PX = 120;
/** Fewest increments in a smooth scroll. */
export const MIN_SCROLL_STEPS = 3;
/** Default wait for an element to appear. */
export const ELEMENT_WAIT_MS = 10000;
/** JPEG quality of a screenshot taken for a model: a retina PNG is megabytes. */
export const SCREENSHOT_JPEG_QUALITY = 70;

/** Recovery guidance when native input would land on another element. */
export const COVERED_TARGET_ERROR = 'Element is covered. Dismiss the overlay and analyze again.';
