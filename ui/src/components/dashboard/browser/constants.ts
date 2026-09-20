/**
 * Every number the browser panel runs on, by name.
 */

/** HTTP statuses the panel reacts to. */
export const Status = {
  /** The browser is gone: close the panel. */
  NOT_FOUND: 404,
} as const;

/** How often the open browser's detail is polled. */
export const POLL_MS = 2000;
/** The window live frames are counted over for the fps readout. */
export const FPS_WINDOW_MS = 1000;
/** How long an optimistic activity line stays before the server's echo replaces it. */
export const OPTIMISTIC_TTL_MS = 3000;
/** Most optimistic activity lines kept at once. */
export const OPTIMISTIC_MAX = 5;
/** How long the copy-id checkmark shows. */
export const COPIED_FLASH_MS = 1200;
