/**
 * Every number the browser connection runs on, by name: WebSocket close codes,
 * timings and limits. Values an operator may tune come from the environment.
 */

/** WebSocket close codes the browser client understands. 4003 stops its reconnects. */
export const CloseCode = {
  /** Same browser reconnected; this socket is the old one. */
  REPLACED: 4000,
  /** No auth message in time. */
  AUTH_TIMEOUT: 4001,
  /** Pongs stopped. */
  PONG_TIMEOUT: 4002,
  /** Bad credential or someone else's browser id: do not retry. */
  REJECTED: 4003,
  /** The server is draining for a restart: retry elsewhere. */
  DRAINING: 4009,
  /** The control plane refused the session (persona cap, enrolment, outage). */
  CONTROL_REJECTED: 4010,
} as const;

/** How often the server pings a browser. */
export const PING_INTERVAL_MS = 20_000;
/** Missed pings tolerated before the socket is closed. */
export const MISSED_PINGS = 4;
/** A socket that has not authenticated by then is closed. */
export const AUTH_DEADLINE_MS = 10_000;

/** Navigation waits on slow sites. */
export const NAVIGATE_TIMEOUT_MS = 90_000;
/**
 * 30s was under what a slow portal page needs: a payer's eligibility screen answers a
 * page read in ~7s, so a type ran over and the agent typed the value a second time.
 */
const DEFAULT_COMMAND_TIMEOUT_MS = 60_000;
/** OYA_COMMAND_TIMEOUT_MS, or the default. */
export const COMMAND_TIMEOUT_MS = Number(process.env.OYA_COMMAND_TIMEOUT_MS) || DEFAULT_COMMAND_TIMEOUT_MS;
/** Browsers one key may hold in the control plane, unless OYA_QUOTA_MAX_BROWSERS says otherwise. */
const DEFAULT_MAX_BROWSERS_PER_KEY = 5000;
/** OYA_QUOTA_MAX_BROWSERS, or the default. */
export const MAX_BROWSERS_PER_KEY = Number(process.env.OYA_QUOTA_MAX_BROWSERS) || DEFAULT_MAX_BROWSERS_PER_KEY;

/** Local commands the desktop may hold open at once; beyond that it is misbehaving. */
export const MAX_LOCAL_COMMANDS = 1024;
/** Cookie changes accepted per message. */
export const MAX_COOKIE_CHANGES = 500;
/** Residential bytes accepted per report (16 GiB); bounds a bad count. */
export const MAX_PROXY_BYTES = 17_179_869_184;
/** Longest desktop_control request id accepted. */
export const MAX_CONTROL_ID = 80;
/** WebSocket close reasons are capped at 123 bytes; persona errors are trimmed to fit. */
export const MAX_CLOSE_REASON = 120;
/** Trailing characters of a rejected key shown in the log. */
export const KEY_HINT_CHARS = 4;
/** Frames per second a browser streams to the live view. */
export const LIVE_VIEW_FPS = 2;
/** Milliseconds per second, for human-readable timeouts. */
export const MS_PER_SECOND = 1000;

/** Providers a browser may claim for itself at enrolment. */
export const CLAIMABLE_PROVIDERS = ['oya-cloud', 'oya-selfhosted', 'oya-desktop'];
/** What an unclaimed browser is. */
export const DEFAULT_PROVIDER = 'oya-desktop';
/** A browser in a sandbox this server provisioned. */
export const CLOUD_PROVIDER = 'oya-cloud';
