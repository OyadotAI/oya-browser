/**
 * Audit log — who did what, to what, and whether it worked.
 *
 * Privileged and state-changing actions only. Per-command volume belongs in
 * usage.js: at 1k-5k browsers, writing a durable row per command would be
 * tens of thousands of inserts a second and would tell you less.
 *
 * Actors are recorded as a key fingerprint, never the key itself.
 */

import { createHash } from 'crypto';
import { appendFile, mkdir } from 'fs/promises';
import { dirname } from 'path';
import { db } from './db.ts';
import { metrics } from './metrics.ts';
import { dataPath } from './paths.ts';
import { AUDIT_FIELD_MAX_CHARS, AUDIT_HISTORY_MAX_ROWS, DB_BATCH_ROWS, FINGERPRINT_HEX_CHARS } from './constants.ts';

const AUDIT_PATH = dataPath('audit.log');

/** Stable, non-reversible actor id. Same shape used for sandbox owner labels. */
export const fingerprint = (key) =>
  key ? createHash('sha256').update(key).digest('hex').slice(0, FINGERPRINT_HEX_CHARS) : null;

const RING_MAX = 500;
const ring = [];
const pending = [];
let flushTimer = null;
const FLUSH_DELAY = 2000;
const PENDING_MAX = 5000; // a burst must not become unbounded memory

/** Flush within 2s, batching whatever arrives meanwhile. */
function scheduleFlush() {
  if (flushTimer) return;
  flushTimer = setTimeout(() => {
    flushTimer = null;
    flush().catch(() => {});
  }, FLUSH_DELAY);
}

/** Append events as JSON lines to audit.log in the data directory. */
async function toFile(batch) {
  await mkdir(dirname(AUDIT_PATH), { recursive: true });
  await appendFile(AUDIT_PATH, batch.map((e) => JSON.stringify(e)).join('\n') + '\n');
}

let warnedFallback = false;

/** Write everything pending to the database, or to the file when there is none or the write fails. */
async function flush() {
  if (!pending.length) return;
  const batch = pending.splice(0, pending.length);
  try {
    if (!db) return await toFile(batch);
    await toDb(batch);
  } catch (e) {
    await fallBackToFile(batch, e);
  }
}

/** Insert the batch into audit_log in chunks; throws on the first chunk that fails. */
async function toDb(batch) {
  for (let i = 0; i < batch.length; i += DB_BATCH_ROWS) {
    const { error } = await db.from('audit_log').insert(batch.slice(i, i + DB_BATCH_ROWS));
    if (error) throw new Error(error.message);
  }
}

/**
 * Losing an audit trail because a table is missing or the database is
 * briefly unreachable is the wrong failure. Fall back to the file rather
 * than drop, and say so once instead of on every flush.
 */
async function fallBackToFile(batch, e) {
  warnFallbackOnce(e);
  try {
    await toFile(batch);
  } catch (fileErr) {
    console.error(`[audit] ${batch.length} events lost, file fallback also failed:`, fileErr.message);
  }
}

/** Say once, not on every flush, that the database write failed and the file is in use. */
function warnFallbackOnce(e) {
  if (warnedFallback) return;
  console.error(`[audit] database write failed (${e.message}) — falling back to ${AUDIT_PATH}`);
  warnedFallback = true;
}

/** One auditable action, as callers pass it to audit(). */
export type AuditEvent = {
  /** e.g. 'key.create', 'browser.provision', 'config.update'. */
  action: string;
  /** Raw API key — fingerprinted, never stored. */
  actorKey?: string | null;
  /** Account id when known. */
  actorUser?: string | null;
  /** 'browser' | 'key' | 'config' | 'sandbox' | 'account'. */
  targetType?: string;
  /** Id of the thing acted on; stored as a string of at most 200 characters. */
  targetId?: string | number | null;
  /** 'ok' | 'denied' | 'error'. */
  outcome?: string;
  /** Small, non-secret detail. */
  meta?: object;
  /** Source of ip and user agent; an Express request or anything shaped like one. */
  req?: {
    /** Read for x-forwarded-for and user-agent. */
    headers?: Record<string, any>;
    /** Fallback source of the ip. */
    socket?: {
      /** Client address. */
      remoteAddress?: string;
    };
  };
};

/** Record one auditable action: kept in memory for recent() and flushed to the database, or the audit file without one. */
export function audit(event: AuditEvent) {
  const row = toRow(event);
  keep(row);
  metrics.auditEvents.inc({ action: row.action, outcome: row.outcome });
  scheduleFlush();
  return row;
}

/** The stored shape of an event: actor fingerprinted, long fields cut, meta copied. */
function toRow({ action, outcome = 'ok', meta, req, ...subject }: AuditEvent) {
  return {
    ts: new Date().toISOString(),
    action,
    ...subjectFields(subject),
    outcome,
    ...requestFields(req),
    meta: meta ? JSON.parse(JSON.stringify(meta)) : null,
  };
}

/** Who acted and on what. */
function subjectFields({ actorKey, actorUser, targetType, targetId }: Partial<AuditEvent>) {
  return {
    actor: fingerprint(actorKey),
    actor_user: actorUser || null,
    target_type: targetType || null,
    target_id: targetId ? String(targetId).slice(0, AUDIT_FIELD_MAX_CHARS) : null,
  };
}

/** Where the action came from: client ip and user agent, when a request was given. */
function requestFields(req: AuditEvent['req']) {
  return {
    ip: req ? req.headers?.['x-forwarded-for']?.split(',')[0].trim() || req.socket?.remoteAddress || null : null,
    user_agent: req?.headers?.['user-agent']?.slice(0, AUDIT_FIELD_MAX_CHARS) || null,
  };
}

/** Keep the event in the recent ring and queue it for the next flush, unless the queue is full. */
function keep(row) {
  ring.push(row);
  if (ring.length > RING_MAX) ring.shift();
  if (pending.length < PENDING_MAX) pending.push(row);
  else if (pending.length === PENDING_MAX)
    console.error('[audit] buffer full — dropping events until the backlog drains');
}

/** Recent events without a round trip. Newest first. */
export function recent({ limit = 100, action, actor, outcome }: any = {}) {
  return ring
    .filter(
      (e) => (!action || e.action === action) && (!actor || e.actor === actor) && (!outcome || e.outcome === outcome),
    )
    .slice(-Math.min(limit, RING_MAX))
    .reverse();
}

/** Durable history. Falls back to the in-memory ring with no database. */
export async function history({ limit = 100, action, actor, since }: any = {}) {
  if (!db) return { source: 'memory', events: recent({ limit, action, actor }) };
  const { data, error } = await historyQuery({ limit, action, actor, since });
  if (error) return { source: 'memory', events: recent({ limit, action, actor }), error: error.message };
  return { source: 'database', events: data };
}

/** Newest audit_log rows first, filtered by whichever of action, actor and since are given. */
function historyQuery({ limit, action, actor, since }) {
  let q = db.from('audit_log').select('*').order('ts', { ascending: false });
  q = q.limit(Math.min(limit, AUDIT_HISTORY_MAX_ROWS));
  if (action) q = q.eq('action', action);
  if (actor) q = q.eq('actor', actor);
  if (since) q = q.gte('ts', since);
  return q;
}

/** Flush before shutdown so the tail of the trail is not lost. */
export async function drain() {
  clearTimeout(flushTimer);
  flushTimer = null;
  await flush();
}
