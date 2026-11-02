/** Window transfer geometry uses Electron screen coordinates (device-independent pixels). */
export const WINDOW_DROP_OFFSET = 24;
/** Maximum hidden-shell preparation time; expiry leaves the original tab untouched. */
export const WINDOW_PAINT_TIMEOUT = 8000;
/** A torn-off tab's window follows the cursor at a display-friendly cadence, without renderer IPC on each move. */
export const DRAG_FRAME_MS = 16;
/** A lost pointer release cannot leave a window following the cursor indefinitely. */
export const DRAG_MAX_MS = 30000;
/** Only the tab strip accepts a tab dropped onto an existing window. */
export const WINDOW_TAB_STRIP_HEIGHT = 44;
/** Why an incognito window was refused: only a signed-in person browses, privately or not. */
export const SIGN_IN_FIRST = 'Sign in before opening an incognito window';
