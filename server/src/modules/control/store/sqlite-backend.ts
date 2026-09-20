/**
 * Local control storage in one SQLite file. Every write runs inside BEGIN
 * IMMEDIATE, so the single writer never sees a half-applied transaction.
 */
import { DatabaseSync } from 'node:sqlite';
import { mkdirSync, chmodSync } from 'node:fs';
import { dirname } from 'node:path';
import { commandsPending, controlPaused } from './errors.ts';
import { claimLock, releaseLockFile, writeLockRecord } from './sqlite-lock.ts';
import { DEFAULT_EVENT_LIMIT, LOCK_HEARTBEAT_MS, PRIVATE_DIR_MODE, PRIVATE_FILE_MODE } from './constants.ts';
import * as sql from './sqlite-sql.ts';

/** An event row as the API returns it. */
const eventOut = (r) => ({
  id: Number(r.seq),
  project: r.project,
  type: r.type,
  sessionId: r.session_id,
  at: Number(r.at),
  detail: typeof r.detail === 'string' ? JSON.parse(r.detail) : r.detail,
});

/** A loaded row; session bodies carry their gate's in-flight count. */
const rowOut = (kind, r) => ({
  id: r.id,
  version: r.version,
  body: kind === 'session' ? { ...JSON.parse(r.body), inFlight: r.inFlight || 0 } : JSON.parse(r.body),
});

/** The WHERE clause and arguments for one load query (by id, project or indexed state). */
function whereOf(q) {
  const clauses: [string, unknown[]][] = [['r.kind = ?', [q.kind]]];
  if (q.id !== undefined) clauses.push(['r.id = ?', [q.id]]);
  if (q.project !== undefined) clauses.push(['r.project = ?', [q.project]]);
  if (q.states) clauses.push([`r.state IN (${sql.placeholders(q.states)})`, q.states]);
  return { where: clauses.map(([clause]) => clause).join(' AND '), args: clauses.flatMap(([, args]) => args) as any[] };
}

/** A gate's columns, mirrored from its session. */
function gateValues(id, x) {
  const c = x.control;
  return [id, x.state, c?.mode || 'agent', c?.holder || null, c?.expiresAt || null, x.fence || 0, x.instance || null];
}

/** Whether a webhook wants an event of this type. */
const wants = (hook, type) => hook.enabled && (!hook.types.length || hook.types.includes(type));

/** A new pending delivery of event `seq` to `hook`. */
function deliveryRow(id, hook, { project, at }, seq) {
  return { id, hook: hook.id, project, eventSeq: seq, at, attempts: 0, nextAt: at, state: 'pending' };
}

/** Whether a gate lets this agent command through: right owner, ready, and agent control or the holder's live takeover. */
function admits(gate, holder, owner) {
  if ((gate.owner && gate.owner !== owner) || gate.state !== 'ready') return false;
  return (
    (gate.mode === 'agent' && !holder) ||
    (gate.mode === 'human' && gate.holder === holder && gate.expiresAt > Date.now())
  );
}

/** Local control storage in one SQLite file, guarded by a heartbeat lock file so only one server writes it. */
export class SqliteBackend {
  /** Open SQLite handle. */
  declare db: DatabaseSync;
  /** Descriptor of the held lock file; undefined once released or when locking is off. */
  declare lockFd: number;
  /** Path of the lock file beside the database. */
  declare lockPath: string;
  /** Interval that keeps refreshing the lock while this process holds it. */
  declare lockTimer: ReturnType<typeof setTimeout>;
  constructor(path, { lock = true } = {}) {
    mkdirSync(dirname(path), { recursive: true, mode: PRIVATE_DIR_MODE });
    if (lock) this.holdLock(`${path}.lock`);
    this.db = new DatabaseSync(path);
    chmodSync(path, PRIVATE_FILE_MODE);
    this.db.exec(sql.SCHEMA);
  }
  /** Claim the lock file and keep refreshing it. */
  private holdLock(lockPath) {
    this.lockPath = lockPath;
    this.lockFd = claimLock(lockPath);
    this.touchLock();
    // Keep proving it: a lock that stops being refreshed is reclaimable.
    this.lockTimer = setInterval(() => this.touchLock(), LOCK_HEARTBEAT_MS);
    this.lockTimer.unref?.();
  }
  /** Rows for each query (by id, project or indexed state); session bodies carry their gate's in-flight count. */
  load(queries) {
    return queries.map((q) => this.loadOne(q));
  }
  /** Rows for one query. */
  private loadOne(q) {
    if (q.states && !q.states.length) return [];
    const { where, args } = whereOf(q);
    return this.db
      .prepare(`${sql.SELECT_ROWS} WHERE ${where}`)
      .all(...args)
      .map((r) => rowOut(q.kind, r));
  }
  /** Apply a transaction's writes and events atomically; `{ ok: false }` if any row's version moved since it was read. */
  commit({ writes = [], events = [] }) {
    const version = this.db.prepare(sql.SELECT_VERSION);
    return this.atomically(() => {
      if (writes.some((w) => ((version.get(w.kind, w.id) as any)?.version ?? 0) !== w.version)) return { ok: false };
      for (const w of writes) this.writeRow(w);
      return { ok: true, events: events.map((e) => this.append(e)) };
    });
  }
  /** Run `work` inside BEGIN IMMEDIATE: committed, or rolled back when it throws or answers `{ ok: false }`. */
  private atomically(work) {
    this.db.exec('BEGIN IMMEDIATE');
    try {
      return this.settle(work());
    } catch (e) {
      this.db.exec('ROLLBACK');
      throw e;
    }
  }
  /** End the open transaction: roll back a `{ ok: false }` result, commit anything else. */
  private settle(result) {
    this.db.exec(result?.ok === false ? 'ROLLBACK' : 'COMMIT');
    return result;
  }
  /** Write or delete one row; a session also updates its gate. */
  private writeRow(w) {
    if (!w.body) return this.deleteRow(w);
    this.db.prepare(sql.UPSERT_ROW).run(w.kind, w.id, w.project, w.state, w.expiresAt, JSON.stringify(w.body));
    if (w.kind === 'session') this.syncGate(w.id, w.body);
  }
  /** Delete one row, and a session's gate with it. */
  private deleteRow(w) {
    this.db.prepare(sql.DELETE_ROW).run(w.kind, w.id);
    if (w.kind === 'session') this.db.prepare(sql.DELETE_GATE).run(w.id);
  }
  /** Command admission reads this per-session gate, so command traffic never rewrites session rows. */
  syncGate(id, x) {
    const gate: any = this.db.prepare(sql.SELECT_GATE_MODE).get(id);
    if (gate?.inFlight > 0 && x.control?.mode === 'human' && gate.mode !== 'human') throw commandsPending();
    this.db.prepare(sql.UPSERT_GATE).run(...gateValues(id, x));
  }
  /** Append an event and queue a pending delivery for every enabled webhook in its project that wants its type. */
  append(e) {
    const seq = Number(
      this.db
        .prepare(sql.INSERT_EVENT)
        .run(e.project, e.type, e.sessionId ?? null, e.at, JSON.stringify(e.detail || {})).lastInsertRowid,
    );
    for (const hook of this.webhooksOf(e.project)) if (wants(hook, e.type)) this.queueDelivery(hook, e, seq);
    return seq;
  }
  /** A project's webhook bodies. */
  private webhooksOf(project) {
    return (this.db.prepare(sql.SELECT_WEBHOOKS).all(project) as any[]).map((row) => JSON.parse(row.body));
  }
  /** Queue a pending delivery of event `seq` to `hook`. */
  private queueDelivery(hook, e, seq) {
    const id = `${hook.id}:${seq}`;
    this.db.prepare(sql.INSERT_DELIVERY).run(id, e.project, JSON.stringify(deliveryRow(id, hook, e, seq)));
  }
  /** A project's events after a sequence number (or the latest `limit`), or specific events by `seqs`. */
  events({ project, after = 0, limit = DEFAULT_EVENT_LIMIT, latest = false, seqs }: any = {}) {
    if (seqs) return this.eventsBySeq(seqs);
    const rows = latest
      ? this.db.prepare(sql.SELECT_LATEST_EVENTS).all(project, limit).reverse()
      : this.db.prepare(sql.SELECT_EVENTS_AFTER).all(project, after, limit);
    return rows.map(eventOut);
  }
  /** Specific events, in sequence order. */
  private eventsBySeq(seqs) {
    if (!seqs.length) return [];
    const query = `SELECT * FROM control_events WHERE seq IN (${sql.placeholders(seqs)}) ORDER BY seq`;
    return this.db
      .prepare(query)
      .all(...seqs)
      .map(eventOut);
  }
  /** Delete expired rows (and gates of pruned sessions) and events older than each project's cutoff. */
  prune(now, cutoffs = {}) {
    this.atomically(() => {
      this.db.prepare(sql.DELETE_EXPIRED_GATES).run(now);
      this.db.prepare(sql.DELETE_EXPIRED_ROWS).run(now);
      for (const [project, before] of Object.entries(cutoffs) as [string, number][])
        this.db.prepare(sql.DELETE_OLD_EVENTS).run(project, before);
    });
  }
  /** Admit an agent command against the session gate and count it in flight; returns the gate's fence, or null for an unknown session. */
  beginCommand(id, holder, owner) {
    const gate: any = this.db.prepare(sql.SELECT_GATE).get(id);
    if (!gate) return null;
    if (!admits(gate, holder, owner)) throw controlPaused();
    this.db.prepare(sql.BEGIN_COMMAND).run(id);
    return gate.fence;
  }
  /** Settle a command admitted under `fence`; a stale fence (session re-placed since) is ignored. */
  finishCommand(id, fence) {
    if (fence !== null) this.db.prepare(sql.FINISH_COMMAND).run(id, fence);
  }
  /** Rewrite the lock file with our pid and the current time, proving the holder is alive. */
  touchLock() {
    if (this.lockFd === undefined) return;
    writeLockRecord(this.lockFd);
  }

  /** Close the database and release the lock file. */
  close() {
    this.db.close();
    this.stopHeartbeat();
    this.releaseLock();
  }
  /** Stop refreshing the lock. */
  private stopHeartbeat() {
    if (!this.lockTimer) return;
    clearInterval(this.lockTimer);
    this.lockTimer = undefined;
  }
  /** Give the lock file up so the next server can start at once. */
  private releaseLock() {
    if (this.lockFd === undefined) return;
    releaseLockFile(this.lockFd, this.lockPath);
    this.lockFd = undefined;
  }
}
