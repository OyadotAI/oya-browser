/**
 * Product analytics to PostHog: best effort, never awaited, dropped on any
 * failure, and a no-op unless POSTHOG_KEY and POSTHOG_HOST are both set. Not
 * evidence; the audit log in platform/audit.ts is. Events are batched and sent
 * to PostHog's /batch/ endpoint by a plain POST, the way audit.ts batches its
 * rows, so nothing here can slow a request or hold the process open.
 */
import {
  ANALYTICS_BATCH_MAX,
  ANALYTICS_FLUSH_MS,
  ANALYTICS_PENDING_MAX,
  MS_PER_HOUR,
  OUTBOUND_TIMEOUT_MS,
} from './constants.ts';
import { metrics } from './metrics.ts';

/** One event as PostHog's batch endpoint takes it. */
type Row = {
  /** `$identify` or the event name. */
  event: string;
  /** Who it is about. */
  distinct_id: string;
  /** The event's properties. */
  properties: Record<string, unknown>;
  /** When it happened. */
  timestamp: string;
};

/** Where events go and with what key. */
type Target = {
  /** The PostHog project's write key. */
  key: string;
  /** The PostHog host, without a trailing slash. */
  host: string;
};

/** Events waiting for the next batch, oldest first. */
const pending: Row[] = [];
/** The pending flush, when one is scheduled. */
let flushTimer: ReturnType<typeof setTimeout> | null = null;
/** When the last failure warning was printed, and how many events went unsent since; a dead PostHog must not fill the log. */
let warnedAt = 0;
/** Events dropped since the last warning. */
let dropped = 0;

/** Where and with what to send: null when analytics is off, read at call time so tests can toggle it. */
function settings(): Target | null {
  const { POSTHOG_KEY: key, POSTHOG_HOST: host } = process.env;
  return key && host ? { key, host: host.replace(/\/+$/, '') } : null;
}

/** Whether analytics is on. */
export const enabled = () => settings() !== null;

/**
 * Every event is sent from this server, so PostHog would place it at the
 * server's address and overwrite the person's real location with the data
 * centre's. Their browser's own events carry the location instead.
 */
const NO_GEOIP = { $geoip_disable: true };

/** Queues one event about `distinctId`. A fingerprint-only id gets no person profile: it is not a person. */
export function capture(distinctId: string, event: string, properties: Record<string, unknown> = {}) {
  if (!enabled()) return;
  const person = properties.$process_person_profile ?? true;
  queue({
    event,
    distinct_id: distinctId,
    properties: { ...properties, ...NO_GEOIP, $process_person_profile: person },
    timestamp: now(),
  });
}

/** Attaches properties such as an email to a person, once known. */
export function identify(distinctId: string, properties: Record<string, unknown>) {
  if (!enabled()) return;
  queue({
    event: '$identify',
    distinct_id: distinctId,
    properties: { $set: properties, ...NO_GEOIP },
    timestamp: now(),
  });
}

/** The current time as PostHog wants it. */
const now = () => new Date().toISOString();

/** Adds a row and sends when the batch is full, else within the flush delay. A full queue drops the newest. */
function queue(row: Row) {
  if (pending.length >= ANALYTICS_PENDING_MAX) return drop(1, 'queue full');
  pending.push(row);
  if (pending.length >= ANALYTICS_BATCH_MAX) return void flush();
  scheduleFlush();
}

/** Sends within the flush delay, batching whatever arrives meanwhile. The timer never keeps the process alive. */
function scheduleFlush() {
  if (flushTimer) return;
  flushTimer = setTimeout(() => {
    flushTimer = null;
    void flush();
  }, ANALYTICS_FLUSH_MS);
  flushTimer.unref?.();
}

/** Sends everything pending in one POST; a failure of any kind is dropped after one warning. */
async function flush() {
  const to = settings();
  if (!pending.length || !to) return void pending.splice(0);
  const batch = pending.splice(0, pending.length);
  try {
    await post(to, batch);
  } catch (err) {
    drop(batch.length, err.message);
  }
}

/** One POST to /batch/, abandoned after the outbound timeout. */
async function post(to: Target, batch: Row[]) {
  const res = await fetch(`${to.host}/batch/`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ api_key: to.key, batch }),
    signal: AbortSignal.timeout(OUTBOUND_TIMEOUT_MS),
  });
  if (!res.ok) throw new Error(`PostHog answered ${res.status}`);
}

/** Counts events that went unsent and says so at most once an hour, so a long outage stays visible without filling the log. */
function drop(count: number, reason: string) {
  dropped += count;
  metrics.analyticsDropped.inc({ reason: reason === 'queue full' ? 'queue_full' : 'posthog' }, count);
  if (Date.now() - warnedAt < MS_PER_HOUR) return;
  warnedAt = Date.now();
  console.error(`[analytics] dropped ${dropped} events: ${reason}`);
  dropped = 0;
}

/** Sends what is pending now, bounded by the outbound timeout, for shutdown. */
export async function drain() {
  if (flushTimer) clearTimeout(flushTimer);
  flushTimer = null;
  await flush();
}

/** Forgets everything pending and the warning, so one test cannot leak into the next. */
export function resetForTests() {
  pending.splice(0);
  if (flushTimer) clearTimeout(flushTimer);
  flushTimer = null;
  warnedAt = 0;
  dropped = 0;
}
