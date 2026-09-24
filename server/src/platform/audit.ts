/**
 * Audit log, who did what, to what, and whether it worked.
 *
 * Privileged and state-changing actions only. Per-command volume belongs in
 * usage.js: at 1k-5k browsers, writing a durable row per command would be
 * tens of thousands of inserts a second and would tell you less.
 *
 * Actors are recorded as a key fingerprint, never the key itself.
 *
 * Every row is hash-linked to the one before it (audit-chain.ts), so a changed
 * or removed row can be found rather than merely disallowed.
 */

import { createHash } from 'crypto';
import { link } from './audit-chain.ts';
import { getConnection } from './storage/index.ts';
import { metrics } from './metrics.ts';
import { AUDIT_FIELD_MAX_CHARS, AUDIT_HISTORY_MAX_ROWS, FINGERPRINT_HEX_CHARS } from './constants.ts';

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
  // A pending flush must not hold the process open; shutdown drains explicitly.
  flushTimer.unref?.();
}

/**
 * Write everything pending to storage. A failed write puts the batch back at
 * the front of the queue, bounded like the queue itself, rather than dropping
 * an audit trail because storage blinked; the next event's flush, or the drain
 * at shutdown, tries it again. It never reschedules itself, so a storage that
 * stays down cannot keep a retry loop, or the process, alive.
 */
async function flush() {
  if (!pending.length) return;
  const batch = pending.splice(0, pending.length);
  await getConnection()
    .upsert('audit_log', batch)
    .catch((e) => requeue(batch, e));
}

/** Puts a batch whose write failed back at the front of the queue, and says so. */
function requeue(batch, e) {
  pending.unshift(...batch.slice(0, PENDING_MAX - pending.length));
  console.error(`[audit] write failed (${e.message}); ${batch.length} events queued for the next flush`);
}

/** One auditable action, as callers pass it to audit(). */
export type AuditEvent = {
  /** e.g. 'key.create', 'browser.provision', 'config.update'. */
  action: string;
  /** Raw API key, fingerprinted, never stored. */
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

/** Record one auditable action: kept in memory for recent() and flushed to storage. */
export function audit(event: AuditEvent) {
  // Linked before anything else sees it: the row that reaches the database and
  // the row kept for recent() are the same row, and both carry the proof.
  const row = link(toRow(event));
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
    console.error('[audit] buffer full, dropping events until the backlog drains');
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

/** Durable history, newest first. Falls back to the in-memory ring when storage cannot be read. */
export async function history({ limit = 100, action, actor, since }: any = {}) {
  try {
    return { source: 'database', events: await historyRows({ limit, action, actor, since }) };
  } catch (e) {
    return { source: 'memory', events: recent({ limit, action, actor }), error: e.message };
  }
}

/** Stored audit rows filtered by whichever of action, actor and since are given, newest first by insertion (id), which ties in ts cannot reorder. */
function historyRows({ limit, action, actor, since }) {
  const where = Object.fromEntries(Object.entries({ action, actor, ts: since && { gte: since } }).filter(([, v]) => v));
  return getConnection().select('audit_log', where, {
    order: ['id', 'desc'],
    limit: Math.min(limit, AUDIT_HISTORY_MAX_ROWS),
  });
}

/** Flush before shutdown so the tail of the trail is not lost. */
export async function drain() {
  clearTimeout(flushTimer);
  flushTimer = null;
  await flush();
}
