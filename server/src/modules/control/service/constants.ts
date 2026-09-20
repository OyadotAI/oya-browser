/**
 * Every number the control service runs on, by name: leases, retention,
 * admission limits, credential lifetimes and validation bounds.
 */

/** Milliseconds per second. */
export const MS_PER_SECOND = 1000;
/** Seconds in a day. */
export const SECONDS_PER_DAY = 86_400;
/** Milliseconds in a day. */
export const DAY_MS = SECONDS_PER_DAY * MS_PER_SECOND;
/** Milliseconds in an hour. */
export const HOUR_MS = 3_600_000;
/** A new session reserves one minute of its hourly rate up front. */
export const MINUTES_PER_HOUR = 60;

/** A new project keeps recordings this many days. */
export const DEFAULT_RECORDING_DAYS = 7;
/** A new project keeps its audit log this many days. */
export const DEFAULT_AUDIT_DAYS = 90;
/** Longest retention a project may set, in days. */
export const MAX_RETENTION_DAYS = 3650;
/** A new project's name ends with this many characters of its id. */
export const PROJECT_NAME_SUFFIX = 6;
/** Hex characters of the key's hash in a project id. */
export const PROJECT_ID_CHARS = 24;
/** Hex characters of the key's hash kept as a project's legacy owner. */
export const LEGACY_OWNER_CHARS = 16;
/** Longest project name. */
export const MAX_PROJECT_NAME = 100;

/** Longest hostname a policy rule may name (DNS's limit). */
export const MAX_HOST_LENGTH = 253;
/** Wildcards per host rule; more makes the matcher backtrack for seconds. */
export const MAX_HOST_WILDCARDS = 3;
/** Rules per host list. */
export const MAX_HOST_RULES = 100;

/** An Idempotency-Key replays its session for this many days. */
const IDEMPOTENCY_DAYS = 7;
/** How long an Idempotency-Key replays its session. */
export const IDEMPOTENCY_WINDOW_MS = IDEMPOTENCY_DAYS * DAY_MS;
/** Longest Idempotency-Key. */
export const MAX_IDEMPOTENCY_KEY = 200;
/** Longest a request may wait in the queue. */
export const MAX_QUEUE_MS = 300_000;
/** Sessions one project may have queued. */
export const MAX_QUEUED = 1000;
/** Browsers per project when the caller sets no cap. */
export const DEFAULT_MAX_CONCURRENT = 5000;

/** A provider slot hold lasts this long. */
export const PROVIDER_HOLD_MS = 180_000;
/** A provisioning worker's lease on its session. */
export const PROVISIONING_LEASE_MS = 180_000;
/** A connected browser's lease on its replica, renewed by heartbeats. */
export const CONNECTED_LEASE_MS = 30_000;

/** Random bytes in tokens, tickets and webhook secrets. */
export const TOKEN_BYTES = 32;
/** A connection ticket is good for this long. */
export const TICKET_TTL_MS = 60_000;
/** Longest credential label. */
export const MAX_LABEL = 100;
/** A share link lasts this long unless the caller says otherwise. */
export const DEFAULT_SHARE_SECONDS = 3600;
/** Shortest share link lifetime. */
export const MIN_SHARE_SECONDS = 60;
/** Longest share link lifetime, in days. */
const MAX_SHARE_DAYS = 30;
/** Longest share link lifetime. */
export const MAX_SHARE_SECONDS = MAX_SHARE_DAYS * SECONDS_PER_DAY;

/** A takeover request pauses the agent this long while the operator's page connects. */
export const TAKEOVER_REQUEST_MS = 10_000;
/** A human's hold on a browser, renewed while they are active. */
export const HUMAN_CONTROL_MS = 300_000;

/** Events in the project overview. */
export const OVERVIEW_EVENTS = 100;
/** Deliveries shown with the webhook settings. */
export const RECENT_DELIVERIES = 20;
