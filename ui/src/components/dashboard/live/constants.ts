/**
 * Every number the live view runs on, by name: input batching, throttles and
 * the thresholds behind its status line.
 */

/** Quiet time after the last keystroke before typed text is sent as one command. */
export const TYPE_FLUSH_MS = 120;
/** A press that moves further than this (page pixels) is a drag, not a click. */
export const DRAG_THRESHOLD_PX = 6;
/** Mouse movement is streamed at most this often, since each move is a request. */
export const MOUSE_MOVE_INTERVAL_MS = 80;
/** Wheel deltas are collected this long before one scroll command is sent. */
export const SCROLL_FLUSH_MS = 40;
/** Pixels per line when the wheel reports in lines (Firefox with a mouse wheel). */
export const WHEEL_LINE_PX = 16;
/** A frame younger than this means the stream is live, even at an idle page's ~1 fps. */
export const LIVE_FRAME_MS = 3000;
/** A frame older than this is called out as stale. */
export const STALE_FRAME_MS = 5000;
/** Milliseconds per second, for the stale-frame age. */
export const MS_PER_SECOND = 1000;
/** `MouseEvent.button` of the primary (usually left) button. */
export const PRIMARY_BUTTON = 0;
/** Divides leftover space evenly between both sides, as object-contain does. */
export const HALVES = 2;

/** `WheelEvent.deltaMode` values that are not plain pixels. */
export const DeltaMode = {
  /** The delta counts lines. */
  LINE: 1,
  /** The delta counts pages. */
  PAGE: 2,
} as const;

/** Keys that are sent as named key presses rather than typed text. */
export const SPECIAL_KEYS: Record<string, string> = {
  Enter: 'Enter',
  Tab: 'Tab',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Escape: 'Escape',
  ArrowUp: 'ArrowUp',
  ArrowDown: 'ArrowDown',
  ArrowLeft: 'ArrowLeft',
  ArrowRight: 'ArrowRight',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
  ' ': 'Space',
};
