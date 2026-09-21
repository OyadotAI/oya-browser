/**
 * Every number the root main-process modules (shell layout, control state, the
 * CDP front door, the dev launcher) run on, by name. main/constants.cjs holds main/'s own.
 */

/** Height of the shell's toolbar above the page. */
const CHROME_HEIGHT = 88;
/** Below this window width the dev panel docks at the bottom instead of the side. */
const COMPACT_WIDTH = 960;
/** Narrowest the side panel gets. */
const PANEL_MIN_WIDTH = 320;
/** Widest the side panel gets. */
const PANEL_MAX_WIDTH = 560;
/** Page width the side panel always leaves. */
const PAGE_MIN_WIDTH = 480;
/** Share of the page height the bottom panel takes in the compact layout. */
const COMPACT_PANEL_SHARE = 0.45;

/** A control request the server has not answered by then fails. */
const CONTROL_REQUEST_TIMEOUT_MS = 12_000;
/** Longest a takeover waits for local automation commands to finish. */
const LOCAL_DRAIN_TIMEOUT_MS = 10_000;
/** How often a takeover checks whether local commands have finished. */
const LOCAL_DRAIN_POLL_MS = 50;
/** How often human control is checked for expiry or renewal. */
const CONTROL_TICK_MS = 1000;
/** Human control is renewed once it has less than this left. */
const CONTROL_RENEW_BEFORE_MS = 240_000;

/** A tab opened through the front door is used after this even if its first load has not settled. */
const FRONT_DOOR_TAB_WAIT_MS = 15_000;
/** Largest CDP message the front door accepts (256 MiB): a full-page screenshot is big. */
const FRONT_DOOR_MAX_PAYLOAD = 268_435_456;
/** JSON-RPC's generic server error, as Chromium answers a refused command. */
const CDP_SERVER_ERROR = -32000;

/** process.argv index of the first argument after `node launch.cjs`. */
const USER_ARGS_START = 2;

/** HTTP statuses the front door answers with, by name. */
const Status = { OK: 200, FORBIDDEN: 403, METHOD_NOT_ALLOWED: 405, BAD_GATEWAY: 502 };

module.exports = {
  CHROME_HEIGHT,
  COMPACT_WIDTH,
  PANEL_MIN_WIDTH,
  PANEL_MAX_WIDTH,
  PAGE_MIN_WIDTH,
  COMPACT_PANEL_SHARE,
  CONTROL_REQUEST_TIMEOUT_MS,
  LOCAL_DRAIN_TIMEOUT_MS,
  LOCAL_DRAIN_POLL_MS,
  CONTROL_TICK_MS,
  CONTROL_RENEW_BEFORE_MS,
  FRONT_DOOR_TAB_WAIT_MS,
  FRONT_DOOR_MAX_PAYLOAD,
  CDP_SERVER_ERROR,
  Status,
  USER_ARGS_START,
};
