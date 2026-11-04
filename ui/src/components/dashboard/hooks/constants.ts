/**
 * Fixed values for the dashboard's hooks.
 */

/**
 * How long the page must keep focus after an `oya://` link before we say the
 * desktop app did not open. When it opens, the page loses focus well before this.
 */
export const APP_OPEN_WAIT_MS = 2500;
/** Match the desktop listener's five-minute lifetime across login redirects. */
export const DESKTOP_RETURN_TTL_MS = 300_000;
/** Native state has 256 bits encoded as lowercase hex. */
export const DESKTOP_STATE_SHAPE = /^[a-f0-9]{64}$/;
