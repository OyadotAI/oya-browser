/**
 * Per-key usage accounting.
 *
 * Aggregated in memory and flushed on an interval, never a write per event,
 * at 1k-5k browsers a durable row per command would be tens of thousands of
 * inserts a second. Counters are bucketed by UTC hour, which is the grain
 * billing and quotas actually need.
 *
 * This is the keyed counterpart to metrics.js: same events, but sliced by
 * api key instead of aggregated across the fleet.
 */

import { getConnection, USAGE_FIELDS } from './storage/index.ts';
import { fingerprint } from './audit.ts';
import { DEFAULT_USAGE_FLUSH_MS, MS_PER_HOUR, MS_PER_SECOND, USAGE_ACTOR_CHARS } from './constants.ts';

/** Every counter a key accumulates per hour; anything else passed to record() is ignored. */
export const FIELDS = USAGE_FIELDS;

/** The start of d's UTC hour, as an ISO string: the bucket key. */
const hourOf = (d = new Date()) =>
  new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate(), d.getUTCHours())).toISOString();

/**
 * Keyed by key fingerprint, never by the key. These rows are persisted, and a
 * metering table is no place to keep a working bearer credential, audit.js
 * has recorded actors this way from the start.
 */
/** fingerprint -> { hour, counters } for the current bucket. */
const buckets = new Map();
/** fingerprint -> Map<browserId, connectedAtMs>, for browser_seconds. */
const live = new Map();
let dirty = false;

/** This fingerprint's bucket for the current hour, started fresh when the hour has rolled over. */
function bucket(id) {
  const hour = hourOf();
  let b = buckets.get(id);
  if (!b || b.hour !== hour) {
    b = { hour, counters: Object.fromEntries(FIELDS.map((f) => [f, 0])) };
    buckets.set(id, b);
  }
  return b;
}

/** Adds n to a counter by fingerprint, ignoring unknown fields and non-numbers. */
function recordId(id, field, n) {
  if (!id || !FIELDS.includes(field) || !Number.isFinite(n)) return;
  bucket(id).counters[field] += n;
  dirty = true;
}

/** Add to a counter for this key's current hour. */
export function record(apiKey, field, n = 1) {
  if (!apiKey) return;
  recordId(fingerprint(apiKey), field, n);
}

/** Starts the browser_seconds clock for a browser and counts it as started. */
export function browserConnected(apiKey, browserId) {
  if (!apiKey || !browserId) return;
  const id = fingerprint(apiKey);
  if (!live.has(id)) live.set(id, new Map());
  live.get(id).set(browserId, Date.now());
  recordId(id, 'browsers_started', 1);
}

/** Stops a browser's clock and books the seconds since it was last settled. */
export function browserDisconnected(apiKey, browserId) {
  if (!apiKey) return;
  const id = fingerprint(apiKey);
  const started = live.get(id)?.get(browserId);
  if (!started) return;
  live.get(id).delete(browserId);
  recordId(id, 'browser_seconds', Math.round((Date.now() - started) / MS_PER_SECOND));
}

/**
 * Browser-seconds for connections still open, so a long-lived browser is
 * accounted for before it disconnects rather than landing in one lump.
 */
function settleOpenBrowsers() {
  const now = Date.now();
  for (const [key, browsers] of live) settleBrowsers(key, browsers, now);
}

/** Book one key's open browsers up to now and restart their clocks. */
function settleBrowsers(key, browsers, now) {
  for (const [id, since] of browsers) {
    const seconds = Math.round((now - since) / MS_PER_SECOND);
    if (seconds <= 0) continue;
    recordId(key, 'browser_seconds', seconds);
    browsers.set(id, now);
  }
}

/** Current hour's counters for one key. */
export function current(apiKey) {
  const id = fingerprint(apiKey);
  const hour = hourOf();
  // A bucket only rolls over on its next write, and a key blocked by an hourly quota writes
  // nothing, so an earlier hour's counters must read as zero here or the block never lifts.
  const b = buckets.get(id)?.hour === hour ? buckets.get(id) : null;
  const openBrowsers = live.get(id)?.size || 0;
  return { hour, openBrowsers, ...(b?.counters || Object.fromEntries(FIELDS.map((f) => [f, 0]))) };
}

/** Every key with activity this hour. */
export function snapshot() {
  return [...buckets.entries()].map(([id, b]) => ({
    actor: id.slice(0, USAGE_ACTOR_CHARS),
    hour: b.hour,
    openBrowsers: live.get(id)?.size || 0,
    ...b.counters,
  }));
}

/** Durable history for one key, newest hour first; the live bucket alone when storage cannot be read. */
export async function history(apiKey, { hours = 24 } = {}) {
  const since = new Date(Date.now() - hours * MS_PER_HOUR).toISOString();
  try {
    const where = { api_key: fingerprint(apiKey), hour: { gte: since } };
    return { source: 'database', rows: await getConnection().select('usage', where, { order: ['hour', 'desc'] }) };
  } catch (e) {
    return { source: 'memory', rows: [current(apiKey)], error: e.message };
  }
}

/**
 * Writes every bucket to the usage table. Counters live in memory and are
 * rewritten whole each flush, so a failed write loses nothing: it stays dirty
 * and the next flush carries it.
 */
async function flush() {
  settleOpenBrowsers();
  if (!dirty) return;
  dirty = false;
  const rows = usageRows();
  if (rows.length) await getConnection().upsert('usage', rows, { update: true }).catch(keepDirty);
}

/** A failed write leaves the counters for the next flush, and says so. */
function keepDirty(e) {
  dirty = true;
  console.error(`[usage] write failed (${e.message}); the next flush retries it`);
}

/** One row per key bucket, stamped with the time of this flush. */
function usageRows() {
  return [...buckets.entries()].map(([id, b]) => ({
    api_key: id,
    hour: b.hour,
    ...b.counters,
    updated_at: new Date().toISOString(),
  }));
}

/**
 * Reload the current hour so a restart mid-hour continues the bucket instead
 * of resetting it, which would otherwise let a quota be evaded by a restart.
 */
export async function restore() {
  const rows = await getConnection().select('usage', { hour: hourOf() });
  // hourOf(), not row.hour: the bucket compares by string, whatever format storage hands back.
  for (const row of rows) buckets.set(row.api_key, { hour: hourOf(), counters: countersFrom(row) });
  if (rows.length) console.log(`[usage] restored ${rows.length} key buckets for the current hour`);
}

/** A stored row's counters, with anything missing or non-numeric as zero. */
function countersFrom(row) {
  return Object.fromEntries(FIELDS.map((f) => [f, Number(row[f]) || 0]));
}

/** How often counters are flushed: OYA_USAGE_FLUSH_MS, or the default. */
const FLUSH_INTERVAL = Number(process.env.OYA_USAGE_FLUSH_MS) || DEFAULT_USAGE_FLUSH_MS;
const timer = setInterval(() => flush().catch(() => {}), FLUSH_INTERVAL);
timer.unref?.();

/** Stops the flush timer and writes out what is left, for shutdown. */
export async function drain() {
  clearInterval(timer);
  await flush();
}

/** Test hook. */
export function reset() {
  buckets.clear();
  live.clear();
  dirty = false;
}
