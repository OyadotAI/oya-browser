/**
 * Numbers and fixed values the desktop shell (src/main/shell/) runs on: the
 * window's size, the toolbar's height, the dev panel's bounds and motion, the
 * control shield's timing, and the shell's colours.
 */

/** Height of the shell's toolbar above the page. */
export const CHROME_HEIGHT = 88;
/** The bookmarks bar's height under the toolbar, when it shows. */
export const BOOKMARKS_BAR_HEIGHT = 30;
/** The find bar's height under the toolbar (and bookmarks bar), while it is open on the shown tab. */
export const FIND_BAR_HEIGHT = 32;
/** Longest find query, in characters: the field and the main process share one limit. */
export { FIND_QUERY_MAX_CHARS } from '../../shared/constants.ts';
/** Most bookmarks the bar carries; the rest are under All bookmarks. */
export const BOOKMARKS_BAR_ITEMS = 40;

/** Below this window width the dev panel docks at the bottom instead of the side. */
export const COMPACT_WIDTH = 960;

/** Narrowest the side panel gets. */
export const PANEL_MIN_WIDTH = 320;

/** Widest the side panel gets. */
export const PANEL_MAX_WIDTH = 560;

/** The side panel's width when the person has not dragged it. */
export const PANEL_WIDTH = 360;

/** Page width the side panel always leaves. */
export const PAGE_MIN_WIDTH = 480;

/** Share of the page height the bottom panel takes in the compact layout. */
export const COMPACT_PANEL_SHARE = 0.45;

/** The main window's first size and the smallest it may shrink to. */
export const WINDOW_SIZE = { width: 1280, height: 860, minWidth: 600, minHeight: 400 } as const;
/** The largest saved window side believed: beyond any real display, so a bigger one in the file is corrupt. */
export const WINDOW_SIZE_MAX = 16384;
/** How much of a restored window must be on a connected display, each way, to be seen and dragged back. */
export const WINDOW_VISIBLE_MIN = 100;
/** The main window's saved bounds, beside config.json. */
export const WINDOW_BOUNDS_FILE = 'window-bounds.json';
/** Where macOS draws the traffic lights on the inset title bar: centred on the tabs (34px tall, ending at the strip's 40px; renderer/tabs.css). */
export const TRAFFIC_LIGHTS = { x: 12, y: 16 } as const;
/** The shell's background in each theme, before its page paints: the launch stage's own edge colour, so nothing flashes. */
export const SHELL_BACKGROUND = { dark: '#0c0f0d', light: '#eef1ed' } as const;
/** A see-through view: the control shield only catches input. */
export const TRANSPARENT = '#00000000';

/** One zoom step, in Chromium zoom levels (each level is 20% larger). */
export const ZOOM_STEP = 0.5;
/** How much larger each Chromium zoom level is than the one below it. */
export const ZOOM_BASE = 1.2;
/** A page at its actual size, as a percentage. */
export const FULL_SIZE_PERCENT = 100;
/** A resize is saved once the drag has been still this long. */
export const PANEL_SAVE_DELAY_MS = 180;
/** How long the dev panel takes to slide open or shut. */
export const PANEL_MOTION_MS = 220;
/** One animation frame of that slide. */
export const PANEL_FRAME_MS = 16;
/** Ease-out cubic: the slide decelerates as it lands. */
export const PANEL_EASE_POWER = 3;

/** How long the shell waits for the renderer to paint a page backdrop before covering the page anyway. */
export const BACKDROP_WAIT_MS = 300;
/** Longest dev-log entry, in characters. */
export const DEV_LOG_MAX_CHARS = 8000;

/** The most elements the control shield outlines after an analysis. */
export const MAX_ANALYSIS_BOXES = 150;
/** How often the control shield re-measures its outlines, so they follow the page as it scrolls and reflows. */
export const SHIELD_TRACK_MS = 33;
/** Below this average brightness (0 black, 1 white) a page is dark, and the shield dims it more deeply. */
export const DARK_PAGE_LUMA = 0.4;
/** The longest element name the companion quotes before it shortens it with an ellipsis. */
export const NARRATION_NAME_MAX = 42;
/** How long outlines are followed: the renderer's longest wait for a scan sweep to end, then its reveal, hold and fade (renderer/core/constants.js), summed. */
export const SHIELD_TRACK_FOR_MS = 10000;

/** Menu items that act on the page, disabled while an agent has control. */
export const HUMAN_MENU_ITEMS: readonly string[] = [
  'browser-new-tab',
  'browser-close-tab',
  'browser-reload',
  'browser-find',
  'browser-find-next',
  'browser-find-previous',
];
/** Cmd/Ctrl + this digit goes to the last tab, whatever their number (Chrome's rule). */
export const LAST_TAB_DIGIT = 9;
/** Named overlays the renderer may raise over the page. */
export const OVERLAYS: readonly string[] = ['legacy', 'shell', 'address', 'notifications', 'profiles'];

/** Native dialog response selecting an explicit takeover. */
export const TAKEOVER_CHOICE = 1;
