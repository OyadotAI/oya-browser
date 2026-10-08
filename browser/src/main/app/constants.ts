/**
 * Numbers and fixed values the app wiring (src/main/app/) runs on: file modes,
 * the saved config's shape, the defaults a fresh install starts from, the
 * updater's schedule and the random bytes the composition root draws.
 */

/**
 * 0600: owner read/write only. Config holds apiKey, a control-plane credential
 * for the whole project, and the default 0644 leaves a file readable by any
 * other local account on a Linux host or in a container.
 */
export const PRIVATE_FILE_MODE = 0o600;
/** Spaces per indent level in JSON written for people to read. */
export const JSON_INDENT = 2;
/** Where a fresh install looks for its workspace. A self-hoster edits the field. */
export const DEFAULT_SERVER_URL = 'wss://oyabrowser.com/ws';
/** Decimal places a fingerprint's noise seeds are shown with. */
export const NOISE_SEED_DIGITS = 6;
/** The scheme one-click sign-in links use. */
export const LINK_SCHEME = 'oya://';

/** How often a packaged app checks for an update (six hours). */
export const UPDATE_CHECK_INTERVAL_MS = 21_600_000;
/** The first check waits this long, so the first window can settle. */
export const UPDATE_FIRST_CHECK_MS = 10_000;

/** Random bytes in the CDP relay's token. */
export const RELAY_TOKEN_BYTES = 32;
/** Random bytes in the isolated world's name. */
export const WORLD_NAME_BYTES = 8;

/**
 * Pages keep rendering while the window is hidden or covered. An agent drives the
 * browser while the person works in other windows, and Chromium stops drawing a
 * window it thinks nobody sees: every click and keystroke then waited 30 seconds for
 * a frame, some ran past the command timeout, and those steps went missing from the
 * recording.
 */
export const KEEP_RENDERING_SWITCHES = [
  'disable-renderer-backgrounding',
  'disable-backgrounding-occluded-windows',
  'disable-background-timer-throttling',
] as const;

/** Bound URLs passed to the operating system's protocol handler. */
export const EXTERNAL_APP_URL_LIMIT = 8192;
/** The explicit affirmative button, never the dialog's default. */
export const OPEN_EXTERNAL_APP_CHOICE = 1;

/** Explicit approval, with Block as the default media permission response. */
export const ALLOW_MEDIA_CHOICE = 1;
