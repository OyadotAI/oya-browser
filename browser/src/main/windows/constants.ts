/** Window transfer geometry uses Electron screen coordinates (device-independent pixels). */
export const WINDOW_DROP_OFFSET = 24;
/** Maximum hidden-shell preparation time; expiry leaves the original tab untouched. */
export const WINDOW_PAINT_TIMEOUT = 8000;
/** Native preview follows the cursor at a display-friendly cadence, without renderer IPC on each move. */
export const DRAG_FRAME_MS = 16;
/** The thumbnail is deliberately smaller than a browser window. */
export const DRAG_PREVIEW_SIZE = { width: 300, height: 228 } as const;
/** Offset keeps the native pointer and its drop target unobscured. */
export const DRAG_PREVIEW_OFFSET = 18;
/** Lost pointer ownership cannot leave a preview window indefinitely. */
export const DRAG_MAX_MS = 30000;
/** Only the tab strip accepts a tab dropped onto an existing window. */
export const WINDOW_TAB_STRIP_HEIGHT = 44;
