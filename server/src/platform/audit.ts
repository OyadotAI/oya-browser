/**
 * Audit log, who did what, to what, and whether it worked.
 *
 * Privileged and state-changing actions only. Per-command volume belongs in
 * usage.js: at 1k-5k browsers, writing a durable row per command would be
 * tens of thousands of inserts a second and would tell you less.
 *
 * Actors are recorded as a key fingerprint, never the key itself. Behind the
 * auth middleware every request carries its project's key, so the fingerprint
 * names the project; the credential that authenticated the caller, the member
 * it belongs to and its role come from req.principal and are kept beside it.
 *
 * Every row is linked to the one before it by a keyed hash (audit-chain.ts),
 * so a changed or removed row can be found rather than merely disallowed.
 * The head of the chain is anchored outside the database (audit-anchor.ts),
 * so rows cut from its end, or a whole chain deleted, can be found too.
 * No event is dropped: past the in-memory bound they go to an overflow file.
 */

import { createHash } from 'crypto';
import { appendFileSync } from 'fs';
import { link, verifyAll } from './audit-chain.ts';
import { checkAnchors, latestAnchors, writeAnchor, type AnchorObjects } from './audit-anchor.ts';
import { dbAuth } from './db.ts';
import { getConnection } from './storage/index.ts';
import { metrics } from './metrics.ts';
import { dataPath } from './paths.ts';
import {
  AUDIT_FIELD_MAX_CHARS,
  AUDIT_FLUSH_DELAY_MS,
  AUDIT_HISTORY_MAX_ROWS,
  AUDIT_OVERFLOW_FILE_MODE,
  AUDIT_PENDING_MAX,
  AUDIT_RING_MAX,
  AUDIT_VERIFY_MAX_ROWS,
  FINGERPRINT_HEX_CHARS,
  auditAnchorBucket,
  auditAnchorIntervalMs,
} from './constants.ts';

/** Stable, non-reversible actor id. Same shape used for sandbox owner labels. */
export const fingerprint = (key) =>
  key ? createHash('sha256').update(key).digest('hex').slice(0, FINGERPRINT_HEX_CHARS) : null;

/** Where events go when storage cannot keep up: one JSON row per line, chain-linked, for an operator to import. */
export const OVERFLOW_FILE = dataPath('audit-overflow.jsonl');

const ring = [];
const pending = [];
let flushTimer = null;
/** The newest row of this process's chain known to be stored: the head an anchor may vouch for. */
let durable = null;
/** The position last anchored, so an idle process does not anchor the same head again. */
let anchoredSeq = 0;

/** Flush within 2s, batching whatever arrives meanwhile. */
function scheduleFlush() {
  if (flushTimer) return;
  flushTimer = setTimeout(() => {
    flushTimer = null;
    flush().catch(() => {});
  }, AUDIT_FLUSH_DELAY_MS);
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
    .then(
      () => stored(batch),
      (e) => requeue(batch, e),
    );
}

/** Remembers the furthest row of a written batch as this chain's durable head. */
function stored(batch) {
  const last = batch.reduce((a, b) => (b.seq > a.seq ? b : a));
  if (!durable || last.seq > durable.seq) durable = last;
}

/** Puts a batch whose write failed back at the front of the queue, and says so. */
function requeue(batch, e) {
  enqueue(batch, true);
  console.error(`[audit] write failed (${e.message}); ${batch.length} events queued for the next flush`);
}

/** Queues rows (at the front for a retried batch) up to the bound; the rest go to the overflow file. */
function enqueue(rows, front = false) {
  const room = Math.max(AUDIT_PENDING_MAX - pending.length, 0);
  if (front) pending.unshift(...rows.slice(0, room));
  else pending.push(...rows.slice(0, room));
  if (rows.length > room) spill(rows.slice(room));
}

/** Appends rows the queue cannot hold to the overflow file, synchronously, and says so loudly. */
function spill(rows) {
  const count = `${rows.length} audit events`;
  try {
    const lines = rows.map((r) => `${JSON.stringify(r)}\n`).join('');
    appendFileSync(OVERFLOW_FILE, lines, { mode: AUDIT_OVERFLOW_FILE_MODE });
    console.error(`[audit] BACKLOG FULL: ${count} written to ${OVERFLOW_FILE}; import them into audit_log`);
  } catch (e) {
    console.error(`[audit] LOST ${count}: backlog full and ${OVERFLOW_FILE} unwritable (${e.message})`);
  }
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
  /** Source of ip, user agent and principal; an Express request or anything shaped like one. */
  req?: {
    /** Read for x-forwarded-for and user-agent. */
    headers?: Record<string, any>;
    /** Who the auth middleware resolved the caller to; its key is the project's, so the rest says who acted. */
    principal?: {
      /** Id of the credential that authenticated the request, when it was a project credential. */
      credentialId?: unknown;
      /** The member the credential was issued to. */
      memberUser?: unknown;
      /** What the credential may do. */
      role?: unknown;
    };
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

/** Where the action came from and who authenticated it: ip, user agent and principal, when a request was given. */
function requestFields(req: AuditEvent['req']) {
  return {
    ip: req ? clientIp(req) : null,
    user_agent: req?.headers?.['user-agent']?.slice(0, AUDIT_FIELD_MAX_CHARS) || null,
    ...principalFields(req?.principal),
  };
}

/**
 * The caller's address: the last X-Forwarded-For hop, the one the ingress
 * appended, as auth/agents.ts clientAddress reads it. Earlier hops are
 * whatever the client claimed and would let it write any address it liked.
 */
function clientIp(req: AuditEvent['req']) {
  const hops = String(req.headers?.['x-forwarded-for'] || '')
    .split(',')
    .map((hop) => hop.trim())
    .filter(Boolean);
  return hops.at(-1) || req.socket?.remoteAddress || null;
}

/** The authenticating credential, its member and role, each null when the request had none. */
function principalFields(principal: NonNullable<AuditEvent['req']>['principal']) {
  const text = (value) => (value ? String(value).slice(0, AUDIT_FIELD_MAX_CHARS) : null);
  return {
    credential_id: text(principal?.credentialId),
    member_user: text(principal?.memberUser),
    actor_role: text(principal?.role),
  };
}

/** Keep the event in the recent ring and queue it for the next flush. */
function keep(row) {
  ring.push(row);
  if (ring.length > AUDIT_RING_MAX) ring.shift();
  enqueue([row]);
}

/** Recent events without a round trip. Newest first. */
export function recent({ limit = 100, action, actor, outcome }: any = {}) {
  return ring
    .filter(
      (e) => (!action || e.action === action) && (!actor || e.actor === actor) && (!outcome || e.outcome === outcome),
    )
    .slice(-Math.min(limit, AUDIT_RING_MAX))
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

/** Flush before shutdown so the tail of the trail is not lost, then anchor the head it reached. */
export async function drain(objects = anchorObjects()) {
  clearTimeout(flushTimer);
  flushTimer = null;
  await flush();
  await anchor(objects).catch(anchorFailed);
}

/** The anchor bucket's client, or null when anchoring is off: no OYA_AUDIT_ANCHOR_BUCKET, or no Supabase storage. */
export const anchorObjects = (): AnchorObjects | null =>
  dbAuth && auditAnchorBucket() ? (dbAuth.storage.from(auditAnchorBucket()) as unknown as AnchorObjects) : null;

/** Says an anchor could not be written; the chain itself is unaffected, so this never fails a caller. */
const anchorFailed = (e) => console.error(`[audit] ${e.message}`);

/** Anchors this process's newest stored row when it moved since the last anchor; answers the head, or null. */
export async function anchor(objects = anchorObjects()) {
  if (!objects || !durable || durable.seq === anchoredSeq) return null;
  const head = { chain: durable.chain, seq: durable.seq, hash: durable.hash };
  await writeAnchor(objects, head);
  anchoredSeq = head.seq;
  return head;
}

/** Anchors every OYA_AUDIT_ANCHOR_INTERVAL_MS for the life of the process; answers the timer, or null when off. */
export function startAnchoring(objects = anchorObjects()) {
  if (!objects) return null;
  const timer = setInterval(() => anchor(objects).catch(anchorFailed), auditAnchorIntervalMs());
  timer.unref?.();
  return timer;
}

/**
 * Verifies the newest stored rows, each chain from the first of them present,
 * logs loudly when a link does not add up, and records the verdict as an
 * audit event of its own. Run once at startup.
 */
export async function verifyStored(objects = anchorObjects()) {
  const rows = await getConnection().select('audit_log', {}, { order: ['id', 'desc'], limit: AUDIT_VERIFY_MAX_ROWS });
  const verdict = await withAnchors(verifyAll(rows, true), objects);
  report(verdict);
  audit({ action: 'audit.verify', targetType: 'audit', outcome: verdict.ok ? 'ok' : 'error', meta: verdict });
  return verdict;
}

/** Logs a broken chain loudly, and unkeyed or unanchored chains as warnings. */
function report(verdict) {
  if (!verdict.ok)
    console.error(`[audit] CHAIN BROKEN at ${verdict.broken.chain}#${verdict.broken.seq}: ${verdict.broken.reason}`);
  if (verdict.unkeyed) console.warn(`[audit] ${verdict.unkeyed} stored rows are on unkeyed (pre-HMAC) chains`);
  if (verdict.anchorError) console.warn(`[audit] anchors not compared: ${verdict.anchorError}`);
}

/**
 * The chain verdict, compared against the newest anchors when there are any:
 * rows cut from the end of a chain, or a chain deleted whole, show only there.
 * An unreadable bucket is reported beside the verdict, not as a break.
 */
async function withAnchors(verdict, objects: AnchorObjects | null) {
  if (!verdict.ok || !objects) return verdict;
  try {
    return { ...verdict, ...(await checkAnchors(await latestAnchors(objects), storedAt)) };
  } catch (e) {
    return { ...verdict, anchors: null, anchorError: e.message };
  }
}

/** One stored row by chain and position, or null. */
const storedAt = async (chain: string, seq: number) =>
  (await getConnection().select('audit_log', { chain, seq }, { limit: 1 }))[0] ?? null;
