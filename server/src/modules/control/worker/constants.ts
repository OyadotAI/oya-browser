/**
 * Every number the background worker runs on, by name: timer periods, lease
 * lengths, batch sizes, backoff bounds and WebSocket close codes.
 */

/** Milliseconds per second, for timestamps and backoff. */
export const MS_PER_SECOND = 1000;
/** One hour, for metering and the delivery backoff cap. */
export const HOUR_MS = 3_600_000;
/** One day: the audit window unit and how long a delivery keeps retrying. */
export const DAY_MS = 86_400_000;

/** How often the tick, credential and lease timers fire. */
export const TICK_INTERVAL_MS = 2000;
/** How often shutdown checks whether in-flight work has finished. */
export const DRAIN_POLL_MS = 50;
/** Gap between maintenance passes. */
export const MAINTENANCE_INTERVAL_MS = 60_000;
/** Audit window for a project that has not set one. */
export const DEFAULT_AUDIT_DAYS = 90;

/** A gateway attachment is renewed once its lease has less than this left… */
export const ATTACHMENT_RENEW_BEFORE_MS = 90_000;
/** …and is extended to this far ahead. */
export const ATTACHMENT_LEASE_MS = 120_000;
/** A gateway hold is renewed once it has less than this left… */
export const HOLD_RENEW_BEFORE_MS = 120_000;
/** …and is extended to this far ahead. */
export const HOLD_LEASE_MS = 180_000;
/** A session this replica holds is renewed once its lease has less than this left… */
export const SESSION_RENEW_BEFORE_MS = 20_000;
/** …and is extended to this far ahead. */
export const SESSION_LEASE_MS = 30_000;

/** Cleanup jobs claimed per tick. */
export const CLEANUP_BATCH = 8;
/** How long a claimed cleanup is this replica's before another may take it. */
export const CLEANUP_LEASE_MS = 120_000;
/** Longest wait between cleanup retries. */
export const CLEANUP_BACKOFF_CAP_MS = 300_000;
/** Cleanup backoff stops doubling after this many attempts. */
export const CLEANUP_BACKOFF_MAX_EXPONENT = 8;

/** Queued browsers started per tick. */
export const PROVISION_BATCH = 8;
/** Where a replayed start request says it came from when OYA_PUBLIC_WS_URL is unset. */
export const DEFAULT_PUBLIC_WS_URL = 'ws://localhost:3100';

/** Webhook deliveries claimed per tick. */
export const DELIVERY_BATCH = 8;
/** How long a claimed delivery is this replica's before another may retry it. */
export const DELIVERY_LEASE_MS = 60_000;
/** Longest wait between delivery retries. */
export const DELIVERY_BACKOFF_CAP_MS = HOUR_MS;
/** Delivery backoff stops doubling after this many attempts. */
export const DELIVERY_BACKOFF_MAX_EXPONENT = 12;
/** A customer webhook that has not answered by then has failed this attempt. */
export const WEBHOOK_TIMEOUT_MS = 10_000;
/** First status that is not a success; a webhook succeeds on 200–299. */
export const REDIRECT_MIN = 300;
/** Lifetime, in seconds, of the live link in a Slack alert. */
export const SLACK_LINK_TTL_S = 3600;

/** First retry waits this long; each attempt doubles it. */
export const BACKOFF_BASE_MS = 1000;
/** Growth per attempt. */
export const BACKOFF_FACTOR = 2;

/** How often a session's cost is accrued. */
export const METER_MS = 30_000;
/** Fraction of the budget that raises the early warning. */
const BUDGET_WARNING = 0.8;
/** Budget fractions that each raise one budget.threshold event. */
export const BUDGET_THRESHOLDS = [BUDGET_WARNING, 1];

/** WebSocket close codes the worker uses. */
export const CloseCode = {
  /** Policy violation: a gateway client's credential failed. */
  POLICY: 1008,
  /** Validation is unavailable right now; desktop browsers reconnect. */
  TRY_AGAIN_LATER: 1013,
  /** The credential is revoked; clients stop reconnecting. */
  REVOKED: 4003,
  /** The control plane stopped the browser. */
  STOPPED: 4008,
} as const;
