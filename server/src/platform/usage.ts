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

import { writeFile, mkdir } from 'fs/promises';
import { dirname } from 'path';
import { db } from './db.ts';
import { fingerprint } from './audit.ts';
import { dataPath } from './paths.ts';
import {
  DB_BATCH_ROWS,
  DEFAULT_USAGE_FLUSH_MS,
  JSON_INDENT,
  MS_PER_HOUR,
  MS_PER_SECOND,
  USAGE_ACTOR_CHARS,
} from './constants.ts';

const USAGE_PATH = dataPath('usage.json');

/** Every counter a key accumulates per hour; anything else passed to record() is ignored. */
export const FIELDS = [
  'commands',
  'command_errors',
  'chat_requests',
  'chat_input_tokens',
  'chat_output_tokens',
  'browser_seconds',
  'browsers_started',
  'cookie_pulls',
  'frames',
  'sandboxes_created',
  'rate_limited',
  'quota_denied',
  'bytes_out',
  'residential_proxy_bytes',
];

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
let warnedFallback = false;

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

/** Durable history for one key. */
export async function history(apiKey, { hours = 24 } = {}) {
  if (!db) return { source: 'memory', rows: [current(apiKey)] };
  const since = new Date(Date.now() - hours * MS_PER_HOUR).toISOString();
  const { data, error } = await queryHistory(fingerprint(apiKey), since);
  if (error) return { source: 'memory', rows: [current(apiKey)], error: error.message };
  return { source: 'database', rows: data };
}

/** This fingerprint's hourly rows since the given time, newest first. */
function queryHistory(id, since) {
  return db.from('usage').select('*').eq('api_key', id).gte('hour', since).order('hour', { ascending: false });
}

/** Writes every bucket to the usage table, or to usage.json when there is no database or the write fails. */
async function flush() {
  settleOpenBrowsers();
  if (!dirty) return;
  dirty = false;
  const rows = usageRows();
  if (!rows.length) return;
  await persist(rows);
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

/** Write the rows to the database, or to the file when there is none or the write fails. */
async function persist(rows) {
  try {
    if (!db) return await toFile(rows);
    await toDb(rows);
  } catch (e) {
    await fallBackToFile(rows, e);
  }
}

/** Mirror the rows to usage.json in the data directory. */
async function toFile(rows) {
  await mkdir(dirname(USAGE_PATH), { recursive: true });
  await writeFile(USAGE_PATH, JSON.stringify(rows, null, JSON_INDENT));
}

/** Upsert the rows in chunks; throws on the first chunk that fails. */
async function toDb(rows) {
  for (let i = 0; i < rows.length; i += DB_BATCH_ROWS) {
    const { error } = await db.from('usage').upsert(rows.slice(i, i + DB_BATCH_ROWS), { onConflict: 'api_key,hour' });
    if (error) throw new Error(error.message);
  }
}

/**
 * Counters live in memory and are rewritten whole each flush, so a failed
 * write is not lost data, but retrying a permanently broken table every
 * minute forever is noise. Mirror to the file and say so once.
 */
async function fallBackToFile(rows, e) {
  if (!warnedFallback) {
    console.error(`[usage] database write failed (${e.message}), falling back to ${USAGE_PATH}`);
    warnedFallback = true;
  }
  await toFile(rows).catch((fileErr) => console.error('[usage] file fallback failed:', fileErr.message));
}

/**
 * Reload the current hour so a restart mid-hour continues the bucket instead
 * of resetting it, which would otherwise let a quota be evaded by a restart.
 */
export async function restore() {
  if (!db) return;
  try {
    await loadCurrentHour();
  } catch (e) {
    console.error('[usage] restore failed:', e.message);
  }
}

/** Load every key's bucket for the current hour from the usage table. */
async function loadCurrentHour() {
  const { data, error } = await db.from('usage').select('*').eq('hour', hourOf());
  if (error || !data) return;
  for (const row of data) buckets.set(row.api_key, { hour: row.hour, counters: countersFrom(row) });
  if (data.length) console.log(`[usage] restored ${data.length} key buckets for the current hour`);
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
