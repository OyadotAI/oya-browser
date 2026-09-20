/**
 * Every number control storage runs on, by name: row retention, the SQLite
 * lock's timings, transaction retries and the Postgres pool.
 */

/** Milliseconds in a day. */
export const DAY_MS = 86_400_000;
/** Milliseconds per second, for human-readable lock ages. */
export const MS_PER_SECOND = 1000;

/** Days a stopped or failed session is kept before the pruner may delete it. */
const TERMINAL_SESSION_DAYS = 7;
/** How long a terminal session outlives its last update. */
export const TERMINAL_SESSION_TTL_MS = TERMINAL_SESSION_DAYS * DAY_MS;
/** Days an idempotency record is kept. */
const IDEMPOTENCY_DAYS = 7;
/** How long an idempotency record outlives its creation. */
export const IDEMPOTENCY_TTL_MS = IDEMPOTENCY_DAYS * DAY_MS;
/** Days a settled webhook delivery is kept. */
const DELIVERY_DAYS = 30;
/** How long a settled delivery outlives its event. */
export const DELIVERY_TTL_MS = DELIVERY_DAYS * DAY_MS;
/** How long an instance row outlives its last lease. */
export const INSTANCE_TTL_MS = DAY_MS;

/** How often the SQLite lock holder rewrites the lock file to prove it is alive. */
export const LOCK_HEARTBEAT_MS = 10_000;
/** A lock nobody has refreshed for this long is stale, whatever pid it names. */
export const LOCK_STALE_MS = 30_000;
/** Directories holding control data are private to the server's user. */
export const PRIVATE_DIR_MODE = 0o700;
/** Control database and lock files are private to the server's user. */
export const PRIVATE_FILE_MODE = 0o600;

/** Events read per call unless the caller asks otherwise. */
export const DEFAULT_EVENT_LIMIT = 500;

/** Attempts at a conflicting transaction before answering storage_contention. */
export const MAX_TX_ATTEMPTS = 30;
/** First retry's backoff ceiling. */
export const BASE_BACKOFF_MS = 5;
/** Each retry multiplies the ceiling by this. */
export const BACKOFF_GROWTH = 2;
/** The backoff ceiling never grows past this. */
export const MAX_BACKOFF_MS = 250;

/** Postgres connections per process unless DATABASE_POOL_MAX says otherwise. */
export const DEFAULT_POOL_MAX = 10;
/** The control store is on the request path; a hung connect must not hang a request. */
export const PG_CONNECT_TIMEOUT_MS = 10_000;
/** Idle Postgres connections are closed after this. */
export const PG_IDLE_TIMEOUT_MS = 30_000;
