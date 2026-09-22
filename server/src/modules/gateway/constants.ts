/**
 * Every number and fixed value the CDP gateway runs on, by name: timings,
 * limits, close codes and the Chrome identity it presents. Values an operator
 * may tune come from the environment.
 */

/** How long a session outlives its client, so a dropped client can resume, unless OYA_SESSION_GRACE_MS says otherwise. */
const DEFAULT_GRACE_MS = 60_000;
/** OYA_SESSION_GRACE_MS, or the default. */
export const GRACE_MS = Number(process.env.OYA_SESSION_GRACE_MS) || DEFAULT_GRACE_MS;
/**
 * Calibration: long enough for ordinary waits (Playwright defaults to 30s),
 * short enough that takeover is never stuck.
 */
const DEFAULT_STUCK_COMMAND_MS = 60000;
/** OYA_STUCK_COMMAND_MS, or the default. */
export const STUCK_COMMAND_MS = Number(process.env.OYA_STUCK_COMMAND_MS) || DEFAULT_STUCK_COMMAND_MS;

/** The CDP message size cap and the dial timeout, which belong to the CDP driver that dials; re-exported so the gateway's names stay. */
export { MAX_PAYLOAD_BYTES, UPSTREAM_CONNECT_MS } from '../../drivers/cdp.ts';
/** Browser messages held for a disconnected client, at most. */
export const MAX_PENDING_TO_CLIENT = 1000;
/** Lease on an attachment record; the control worker renews it while the session lives. */
export const ATTACHMENT_LEASE_MS = 120000;
/** Milliseconds per second, for session durations. */
export const MS_PER_SECOND = 1000;
/** Length of "Bearer ", stripped from an Authorization header. */
export const BEARER_PREFIX_LENGTH = 7;

/** WebSocket close codes the gateway sends a client. */
export const CloseCode = {
  /** Normal end of a session. */
  GOING_AWAY: 1001,
  /** A command was refused: access paused or revoked, or the command was invalid. */
  POLICY_VIOLATION: 1008,
} as const;

/** Status codes at or above this are server faults, whose messages are not shown to callers. */
export const SERVER_ERROR_MIN = 500;

/** The /json/version identity of a real Chrome, so clients treat the gateway as one. */
export const CHROME_VERSION = {
  Browser: 'Chrome/126.0.0.0',
  'Protocol-Version': '1.3',
  'User-Agent':
    'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 ' +
    '(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36',
  'V8-Version': '12.6.228.9',
  'WebKit-Version': '537.36',
};

/** First provider cooldown after a failure; doubles per consecutive failure. */
export const COOLDOWN_BASE_MS = 5_000;
/** Longest a failing provider is skipped (5 minutes). */
export const COOLDOWN_MAX_MS = 300_000;
/** Cooldown grows by this factor per consecutive failure. */
export const BACKOFF_FACTOR = 2;
/** Weight of the previous average in the latency EWMA. */
export const LATENCY_KEEP = 0.7;
/** Weight of a new measurement in the latency EWMA. */
export const LATENCY_NEW = 0.3;
/** Default sessions one provider serves at once. */
export const DEFAULT_MAX_CONCURRENT = 10;
/** Default priority; lower wins. */
export const DEFAULT_PRIORITY = 100;
/** How long acquire() waits for a free slot by default. */
export const DEFAULT_QUEUE_MS = 30_000;
/** Providers tried before acquire() gives up. */
export const DEFAULT_ATTEMPTS = 3;
/** Pause before asking the control plane for provider capacity again. */
export const CAPACITY_RETRY_MS = 100;

/** Recording: frames kept per session (~15 min at 2fps), unless OYA_RECORD_MAX_FRAMES says otherwise. */
const DEFAULT_RECORD_MAX_FRAMES = 1800;
/** OYA_RECORD_MAX_FRAMES, or the default. */
export const RECORD_MAX_FRAMES = Number(process.env.OYA_RECORD_MAX_FRAMES) || DEFAULT_RECORD_MAX_FRAMES;
/** Recording: bytes kept per session (100 MiB), unless OYA_RECORD_MAX_BYTES says otherwise. */
const DEFAULT_RECORD_MAX_BYTES = 104_857_600;
/** OYA_RECORD_MAX_BYTES, or the default. */
export const RECORD_MAX_BYTES = Number(process.env.OYA_RECORD_MAX_BYTES) || DEFAULT_RECORD_MAX_BYTES;
/** Recording: JPEG quality, unless OYA_RECORD_QUALITY says otherwise. */
const DEFAULT_RECORD_QUALITY = 40;
/** OYA_RECORD_QUALITY, or the default. */
export const RECORD_QUALITY = Number(process.env.OYA_RECORD_QUALITY) || DEFAULT_RECORD_QUALITY;
/** Recording: keep every Nth screencast frame, unless OYA_RECORD_EVERY_NTH says otherwise. */
const DEFAULT_RECORD_EVERY_NTH = 5;
/** OYA_RECORD_EVERY_NTH, or the default. */
export const RECORD_EVERY_NTH = Number(process.env.OYA_RECORD_EVERY_NTH) || DEFAULT_RECORD_EVERY_NTH;
/** Widest screencast frame Chrome is asked for. */
export const SCREENCAST_MAX_WIDTH = 1280;
/** Tallest screencast frame Chrome is asked for. */
export const SCREENCAST_MAX_HEIGHT = 800;
/** Digits in a frame file name (000042.jpg). */
export const FRAME_NAME_DIGITS = 6;
/** Days a recording is kept when its project sets none. */
export const DEFAULT_RECORDING_DAYS = 7;
/** Milliseconds per day. */
export const MS_PER_DAY = 86400000;
/** A session id: a UUID. */
export const SESSION_ID_PATTERN = /^[0-9a-f-]{36}$/i;

/** Suffix of a saved profile file. */
export const PROFILE_SUFFIX = '.enc';
/** Directory mode for profile and recording spools: owner only. */
export const PRIVATE_DIR_MODE = 0o700;
/** File mode for a sealed profile: owner read/write only. */
export const PRIVATE_FILE_MODE = 0o600;
