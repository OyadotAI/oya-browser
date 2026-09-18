/**
 * Numbers and fixed values the desktop shell (main/shell/) runs on: the
 * window's size, the dev panel's limits and motion, and the shell's colours.
 */

/** The main window's first size and the smallest it may shrink to. */
const WINDOW_SIZE = { width: 1280, height: 860, minWidth: 600, minHeight: 400 };
/** Where macOS draws the traffic lights on the inset title bar. */
const TRAFFIC_LIGHTS = { x: 12, y: 12 };
/** The shell's background in each theme, before its page paints. */
const SHELL_BACKGROUND = { dark: '#1b1e1c', light: '#f5f5f2' };
/** A see-through view: the control shield only catches input. */
const TRANSPARENT = '#00000000';

/** The dev panel's width before the person resizes it. */
const DEFAULT_PANEL_WIDTH = 360;
/** The narrowest the dev panel may be dragged. */
const MIN_PANEL_WIDTH = 320;
/** The widest the dev panel may be dragged. */
const MAX_PANEL_WIDTH = 560;
/** A resize is saved once the drag has been still this long. */
const PANEL_SAVE_DELAY_MS = 180;
/** How long the dev panel takes to slide open or shut. */
const PANEL_MOTION_MS = 220;
/** One animation frame of that slide. */
const PANEL_FRAME_MS = 16;
/** Ease-out cubic: the slide decelerates as it lands. */
const PANEL_EASE_POWER = 3;

/** How long the shell waits for the renderer to paint a page backdrop before covering the page anyway. */
const BACKDROP_WAIT_MS = 300;
/** Longest dev-log entry, in characters. */
const DEV_LOG_MAX_CHARS = 8000;

/** Menu items that act on the page, disabled while an agent has control. */
const HUMAN_MENU_ITEMS = ['browser-new-tab', 'browser-close-tab', 'browser-reload'];
/** Named overlays the renderer may raise over the page. */
const OVERLAYS = ['legacy', 'shell'];

module.exports = {
  WINDOW_SIZE,
  TRAFFIC_LIGHTS,
  SHELL_BACKGROUND,
  TRANSPARENT,
  DEFAULT_PANEL_WIDTH,
  MIN_PANEL_WIDTH,
  MAX_PANEL_WIDTH,
  PANEL_SAVE_DELAY_MS,
  PANEL_MOTION_MS,
  PANEL_FRAME_MS,
  PANEL_EASE_POWER,
  BACKDROP_WAIT_MS,
  DEV_LOG_MAX_CHARS,
  HUMAN_MENU_ITEMS,
  OVERLAYS,
};
