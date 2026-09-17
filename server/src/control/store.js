/**
 * Row-level control storage. Every entity is a versioned row; a transaction writes only the rows it
 * changed, with compare-and-swap on each row's version. Events are an append-only log, and webhook
 * deliveries are created atomically with the events that trigger them.
 *
 * Transactions may await their own reads (tx.get/list) but must perform no other I/O: they are re-run on conflict.
 * Invariants that span rows (capacity, provider holds) take a lock row: tx.lock bumps its version, so concurrent
 * transactions that decide from the same set of rows serialize on it.
 */
import { DatabaseSync } from 'node:sqlite';
import { mkdirSync, openSync, closeSync, unlinkSync, readFileSync, chmodSync, writeSync, ftruncateSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { db } from '../db.js';
import { pgRemote } from './pg-client.js';

const DAY = 86400000;
const TERMINAL = ['stopped', 'failed'];
const fail = (code, message, status) => Object.assign(new Error(message), { code, status });
/** Which body field each kind indexes in the `state` column. */
const STATE_FIELD = { credential: 'role', hold: 'resource', attachment: 'instance', membership: 'userId', project: 'ownerUser', recording: 'owner' };

/** Indexed columns and the time after which the pruner may delete the row. */
export function columns(kind, body) {
  const expiresAt = kind === 'session' ? (TERMINAL.includes(body.state) ? (body.updatedAt || body.createdAt) + 7 * DAY : null)
    : ['ticket', 'invite', 'hold', 'slack_state'].includes(kind) ? body.expiresAt
    : kind === 'attachment' ? body.leaseUntil
    : kind === 'idempotency' ? body.createdAt + 7 * DAY
    : kind === 'delivery' ? (body.state === 'pending' ? null : body.at + 30 * DAY)
    : kind === 'instance' ? body.leaseUntil + DAY
    : null;
  return { project: body.project ?? null, state: body[STATE_FIELD[kind] || 'state'] ?? null, expiresAt: expiresAt ?? null };
}
const matches = (kind, body, filter) => {
  const c = columns(kind, body);
  return (filter.project === undefined || c.project === filter.project) && (!filter.states || filter.states.includes(c.state));
};
const eventOut = r => ({ id: Number(r.seq), project: r.project, type: r.type, sessionId: r.session_id, at: Number(r.at), detail: typeof r.detail === 'string' ? JSON.parse(r.detail) : r.detail });

/**
 * One SQLite control database allows exactly one writer, and the lock has to
 * survive the holder being killed.
 *
 * A pid is not an identity here. In a container the server is pid 1, so after an
 * unclean stop the lock names pid 1 and the *next* container — also pid 1 — finds
 * that pid alive, concludes another server holds the lock, and refuses to start.
 * The deployment then never recovers. So the holder proves it is alive by
 * touching the lock instead: a lock nobody has refreshed is stale, whatever pid
 * it names, while a second live replica keeps its own lock fresh and is still
 * correctly turned away.
 */
const LOCK_HEARTBEAT_MS = 10_000;
const LOCK_STALE_MS = 30_000;

export class SqliteBackend {
  constructor(path, { lock = true } = {}) {
    mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
    if (lock) {
      this.lockPath = `${path}.lock`;
      const claim = () => {
        this.lockFd = openSync(this.lockPath, 'wx', 0o600);
        this.touchLock();
      };
      try { claim(); }
      catch (err) {
        if (err.code !== 'EEXIST') throw err;
        let held;
        try { held = JSON.parse(readFileSync(this.lockPath, 'utf8')); } catch { held = null; }
        const age = held?.at ? Date.now() - Number(held.at) : Infinity;
        if (age < LOCK_STALE_MS) {
          throw new Error('Local control database is already in use; use Postgres for multiple replicas'
            + ` (lock refreshed ${Math.round(age / 1000)}s ago; it goes stale after ${LOCK_STALE_MS / 1000}s`
            + ' if that server is gone)');
        }
        unlinkSync(this.lockPath);
        claim();
      }
      // Keep proving it: a lock that stops being refreshed is reclaimable.
      this.lockTimer = setInterval(() => this.touchLock(), LOCK_HEARTBEAT_MS);
      this.lockTimer.unref?.();
    }
    this.db = new DatabaseSync(path);
    chmodSync(path, 0o600);
    this.db.exec(`PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA busy_timeout=5000;
      CREATE TABLE IF NOT EXISTS control_rows (kind TEXT NOT NULL, id TEXT NOT NULL, project TEXT, state TEXT, expires_at INTEGER, body TEXT NOT NULL, version INTEGER NOT NULL, PRIMARY KEY (kind, id));
      CREATE INDEX IF NOT EXISTS control_rows_project ON control_rows (kind, project);
      CREATE INDEX IF NOT EXISTS control_rows_state ON control_rows (kind, state);
      CREATE INDEX IF NOT EXISTS control_rows_expiry ON control_rows (expires_at) WHERE expires_at IS NOT NULL;
      CREATE TABLE IF NOT EXISTS control_events (seq INTEGER PRIMARY KEY AUTOINCREMENT, project TEXT NOT NULL, type TEXT NOT NULL, session_id TEXT, at INTEGER NOT NULL, detail TEXT NOT NULL);
      CREATE INDEX IF NOT EXISTS control_events_project ON control_events (project, seq);
      CREATE TABLE IF NOT EXISTS control_gates (id TEXT PRIMARY KEY, state TEXT NOT NULL, mode TEXT NOT NULL, holder TEXT, expiresAt INTEGER, fence INTEGER NOT NULL, inFlight INTEGER NOT NULL DEFAULT 0, owner TEXT);`);
  }
  load(queries) {
    return queries.map(q => {
      if (q.states && !q.states.length) return [];
      const where = ['r.kind = ?'], args = [q.kind];
      if (q.id !== undefined) { where.push('r.id = ?'); args.push(q.id); }
      if (q.project !== undefined) { where.push('r.project = ?'); args.push(q.project); }
      if (q.states) { where.push(`r.state IN (${q.states.map(() => '?').join(',')})`); args.push(...q.states); }
      return this.db.prepare(`SELECT r.id, r.version, r.body, g.inFlight FROM control_rows r LEFT JOIN control_gates g ON r.kind = 'session' AND g.id = r.id WHERE ${where.join(' AND ')}`).all(...args)
        .map(r => ({ id: r.id, version: r.version, body: q.kind === 'session' ? { ...JSON.parse(r.body), inFlight: r.inFlight || 0 } : JSON.parse(r.body) }));
    });
  }
  commit({ writes = [], events = [] }) {
    const version = this.db.prepare('SELECT version FROM control_rows WHERE kind = ? AND id = ?');
    this.db.exec('BEGIN IMMEDIATE');
    try {
      if (writes.some(w => (version.get(w.kind, w.id)?.version ?? 0) !== w.version)) { this.db.exec('ROLLBACK'); return { ok: false }; }
      for (const w of writes) {
        if (!w.body) {
          this.db.prepare('DELETE FROM control_rows WHERE kind = ? AND id = ?').run(w.kind, w.id);
          if (w.kind === 'session') this.db.prepare('DELETE FROM control_gates WHERE id = ?').run(w.id);
          continue;
        }
        this.db.prepare('INSERT INTO control_rows VALUES (?,?,?,?,?,?,1) ON CONFLICT (kind, id) DO UPDATE SET project = excluded.project, state = excluded.state, expires_at = excluded.expires_at, body = excluded.body, version = version + 1')
          .run(w.kind, w.id, w.project, w.state, w.expiresAt, JSON.stringify(w.body));
        if (w.kind === 'session') this.syncGate(w.id, w.body);
      }
      const seqs = events.map(e => this.append(e));
      this.db.exec('COMMIT');
      return { ok: true, events: seqs };
    } catch (e) { this.db.exec('ROLLBACK'); throw e; }
  }
  /** Command admission reads this per-session gate, so command traffic never rewrites session rows. */
  syncGate(id, x) {
    const gate = this.db.prepare('SELECT mode, inFlight FROM control_gates WHERE id = ?').get(id);
    if (gate?.inFlight > 0 && x.control?.mode === 'human' && gate.mode !== 'human') throw fail('commands_pending', 'In-flight commands must settle before takeover', 409);
    this.db.prepare('INSERT INTO control_gates VALUES (?,?,?,?,?,?,0,?) ON CONFLICT (id) DO UPDATE SET owner = excluded.owner, state = excluded.state, mode = excluded.mode, holder = excluded.holder, expiresAt = excluded.expiresAt, inFlight = CASE WHEN fence <> excluded.fence THEN 0 ELSE inFlight END, fence = excluded.fence')
      .run(id, x.state, x.control?.mode || 'agent', x.control?.holder || null, x.control?.expiresAt || null, x.fence || 0, x.instance || null);
  }
  append(e) {
    const seq = Number(this.db.prepare('INSERT INTO control_events (project, type, session_id, at, detail) VALUES (?,?,?,?,?)').run(e.project, e.type, e.sessionId ?? null, e.at, JSON.stringify(e.detail || {})).lastInsertRowid);
    for (const row of this.db.prepare("SELECT body FROM control_rows WHERE kind = 'webhook' AND project = ?").all(e.project)) {
      const hook = JSON.parse(row.body), id = `${hook.id}:${seq}`;
      if (!hook.enabled || (hook.types.length && !hook.types.includes(e.type))) continue;
      this.db.prepare("INSERT INTO control_rows VALUES ('delivery', ?, ?, 'pending', NULL, ?, 1)").run(id, e.project, JSON.stringify({ id, hook: hook.id, project: e.project, eventSeq: seq, at: e.at, attempts: 0, nextAt: e.at, state: 'pending' }));
    }
    return seq;
  }
  events({ project, after = 0, limit = 500, latest = false, seqs } = {}) {
    if (seqs) return seqs.length ? this.db.prepare(`SELECT * FROM control_events WHERE seq IN (${seqs.map(() => '?').join(',')}) ORDER BY seq`).all(...seqs).map(eventOut) : [];
    const rows = latest
      ? this.db.prepare('SELECT * FROM control_events WHERE project = ? ORDER BY seq DESC LIMIT ?').all(project, limit).reverse()
      : this.db.prepare('SELECT * FROM control_events WHERE project = ? AND seq > ? ORDER BY seq LIMIT ?').all(project, after, limit);
    return rows.map(eventOut);
  }
  /** Delete expired rows (and gates of pruned sessions) and events older than each project's cutoff. */
  prune(now, cutoffs = {}) {
    this.db.exec('BEGIN IMMEDIATE');
    try {
      this.db.prepare("DELETE FROM control_gates WHERE id IN (SELECT id FROM control_rows WHERE kind = 'session' AND expires_at < ?)").run(now);
      this.db.prepare('DELETE FROM control_rows WHERE expires_at < ?').run(now);
      for (const [project, before] of Object.entries(cutoffs)) this.db.prepare('DELETE FROM control_events WHERE project = ? AND at < ?').run(project, before);
      this.db.exec('COMMIT');
    } catch (e) { this.db.exec('ROLLBACK'); throw e; }
  }
  beginCommand(id, holder, owner) {
    const gate = this.db.prepare('SELECT * FROM control_gates WHERE id = ?').get(id);
    if (!gate) return null;
    if ((gate.owner && gate.owner !== owner) || gate.state !== 'ready' || !((gate.mode === 'agent' && !holder) || (gate.mode === 'human' && gate.holder === holder && gate.expiresAt > Date.now()))) throw fail('control_paused', 'Agent commands are paused for human takeover', 409);
    this.db.prepare('UPDATE control_gates SET inFlight = inFlight + 1 WHERE id = ?').run(id);
    return gate.fence;
  }
  finishCommand(id, fence) {
    if (fence !== null) this.db.prepare('UPDATE control_gates SET inFlight = max(0, inFlight - 1) WHERE id = ? AND fence = ?').run(id, fence);
  }
  touchLock() {
    if (this.lockFd === undefined) return;
    try {
      const record = `${JSON.stringify({ pid: process.pid, at: Date.now() })}\n`;
      writeSync(this.lockFd, record, 0);
      ftruncateSync(this.lockFd, Buffer.byteLength(record));
    } catch { /* a lock we cannot refresh will be reclaimed; that is the intent */ }
  }

  close() {
    this.db.close();
    if (this.lockTimer) { clearInterval(this.lockTimer); this.lockTimer = undefined; }
    if (this.lockFd !== undefined) {
      closeSync(this.lockFd);
      try { unlinkSync(this.lockPath); } catch { /* already gone */ }
      this.lockFd = undefined;
    }
  }
}

/** The same contract over Supabase RPC (server/migrations/008_durable_control.sql). */
export class RemoteBackend {
  constructor(client) { this.client = client; }
  async call(name, args) {
    const { data, error } = await this.client.rpc(name, args);
    if (!error) return { data };
    const message = String(error.message || '');
    if (message.includes('commands_pending')) throw fail('commands_pending', 'In-flight commands must settle before takeover', 409);
    if (message.includes('control_paused')) throw fail('control_paused', 'Agent commands are paused for human takeover', 409);
    if (message.includes('control_conflict')) return { conflict: true };
    throw Object.assign(fail('storage_unavailable', 'Control storage unavailable', 503), { cause: error });
  }
  async load(queries) { return (await this.call('control_load', { queries })).data; }
  async commit({ writes = [], events = [] }) {
    const { data, conflict } = await this.call('control_commit', { writes, events });
    return conflict ? { ok: false } : data;
  }
  async events({ project = null, after = 0, limit = 500, latest = false, seqs = null } = {}) {
    return (await this.call('control_read_events', { target_project: project, after_seq: after, lim: limit, latest, seqs })).data;
  }
  async prune(now, cutoffs = {}) { await this.call('control_prune', { now_ms: now, cutoffs }); }
  async beginCommand(id, holder, owner) { return (await this.call('control_begin', { session_id: id, actor: holder, caller_instance: owner })).data; }
  async finishCommand(id, fence) { if (fence !== null) await this.call('control_finish', { session_id: id, generation: fence }); }
  close() {}
}

/** Per-key FIFO mutex for transactions in this process. */
class KeyedMutex {
  constructor() { this.tails = new Map(); }
  acquire(key) {
    const prior = this.tails.get(key);
    let release;
    const held = new Promise(resolve => { release = resolve; });
    const tail = (prior || Promise.resolve()).then(() => held);
    this.tails.set(key, tail);
    void tail.then(() => { if (this.tails.get(key) === tail) this.tails.delete(key); });
    return (prior || Promise.resolve()).then(() => ({ release, waited: !!prior }));
  }
}

export class Tx {
  constructor(backend, mutex = null) { this.backend = backend; this.mutex = mutex; this.rows = new Map(); this.events = []; this.releases = new Map(); this.fetched = new Set(); }
  /** Load several queries in one round trip; later get/list calls for them are served from this transaction. */
  async prefetch(queries) {
    const found = await this.backend.load(queries);
    queries.forEach((q, i) => {
      if (q.id !== undefined) return void this.#entry(q.kind, found[i][0] || { id: q.id, version: 0, body: null });
      for (const row of found[i]) this.#entry(q.kind, row);
      const { kind, ...filter } = q;
      this.fetched.add(JSON.stringify([kind, filter]));
    });
  }
  /** Take this process's lock on a row before reading anything, so a following prefetch sees its latest version. */
  async acquire(kind, id) {
    const key = [kind, id].join(' ');
    if (this.mutex && !this.releases.has(key)) this.releases.set(key, (await this.mutex.acquire(key)).release);
  }
  #entry(kind, row) {
    const key = `${kind} ${row.id}`;
    if (!this.rows.has(key)) this.rows.set(key, { kind, id: row.id, version: row.version, original: JSON.stringify(row.body), body: row.body });
    return this.rows.get(key);
  }
  async get(kind, id) { return (await this.getMany(kind, [id]))[0] ?? null; }
  /** One round trip for several rows; missing rows are omitted. */
  async getMany(kind, ids) {
    const missing = [...new Set(ids)].filter(id => !this.rows.has(`${kind} ${id}`));
    if (missing.length) {
      const found = await this.backend.load(missing.map(id => ({ kind, id })));
      missing.forEach((id, i) => this.#entry(kind, found[i][0] || { id, version: 0, body: null }));
    }
    return ids.map(id => this.rows.get(`${kind} ${id}`).body).filter(Boolean);
  }
  /** Rows matching an indexed filter, merged with this transaction's own changes. */
  async list(kind, filter = {}) {
    if (!this.fetched.has(JSON.stringify([kind, filter]))) {
      const [found] = await this.backend.load([{ kind, ...filter }]);
      for (const row of found) this.#entry(kind, row);
    }
    return [...this.rows.values()].filter(r => r.kind === kind && r.body && matches(kind, r.body, filter)).map(r => r.body);
  }
  /** Insert a new row or replace one this transaction has read. */
  put(kind, id, body) {
    const entry = this.rows.get(`${kind} ${id}`);
    if (entry) entry.body = body;
    else this.rows.set(`${kind} ${id}`, { kind, id, version: 0, original: 'null', body });
    return body;
  }
  async delete(kind, id) { await this.get(kind, id); this.rows.get(`${kind} ${id}`).body = null; }
  /**
   * Serialize with every other transaction that locks the same row, creating it if needed. Lockers in this
   * process queue on a mutex held until commit, so under a burst only other replicas can conflict.
   */
  async lock(kind, id) {
    const key = [kind, id].join(' ');
    if (this.mutex && !this.releases.has(key)) {
      const { release, waited } = await this.mutex.acquire(key);
      this.releases.set(key, release);
      const cached = this.rows.get(key);
      // A local transaction may have committed this row while we waited; lock on the version it left.
      if (waited && cached?.version) {
        const [[fresh]] = await this.backend.load([{ kind, id }]);
        if (fresh && fresh.version !== cached.version) {
          for (const field of Object.keys(cached.body)) delete cached.body[field];
          Object.assign(cached.body, fresh.body);
          Object.assign(cached, { version: fresh.version, original: JSON.stringify(fresh.body) });
        }
      }
    }
    if (!(await this.get(kind, id))) this.put(kind, id, { id });
    this.rows.get(key).locked = true;
  }
  release() { for (const release of this.releases.values()) release(); this.releases.clear(); }
  /** Rows of a kind this transaction changed, with their state as read. */
  changes(kind) {
    return [...this.rows.values()].filter(r => r.kind === kind && r.body && JSON.stringify(r.body) !== r.original).map(r => ({ before: JSON.parse(r.original), after: r.body }));
  }
  emit(project, type, sessionId = null, detail = {}) {
    const at = Date.now();
    this.events.push({ project, type, sessionId, at, detail });
    const session = sessionId && this.rows.get(`session ${sessionId}`)?.body;
    if (session) session.updatedAt = at;
  }
  plan() {
    const writes = [];
    for (const r of this.rows.values()) {
      if ((JSON.stringify(r.body) === r.original && !r.locked) || (!r.body && !r.version)) continue;
      let body = r.body;
      if (body && r.kind === 'session') { const { inFlight, ...stored } = body; body = stored; }
      writes.push({ kind: r.kind, id: r.id, version: r.version, body, ...(body ? columns(r.kind, body) : {}) });
    }
    return { writes, events: this.events };
  }
}

export class ControlStore {
  constructor({ path, remote = null, lock = true } = {}) {
    this.backend = remote ? new RemoteBackend(remote) : new SqliteBackend(path, { lock });
    this.serial = !remote;
    this.tail = Promise.resolve();
    this.locks = new KeyedMutex();
  }
  async get(kind, id) { return (await this.backend.load([{ kind, id }]))[0][0]?.body ?? null; }
  async list(kind, filter = {}) { return (await this.backend.load([{ kind, ...filter }]))[0].map(r => r.body); }
  async load(queries) { return this.backend.load(queries); }
  async events(options) { return this.backend.events(options); }
  async prune(now, cutoffs) { return this.backend.prune(now, cutoffs); }
  transact(fn) {
    const run = async () => {
      for (let attempt = 0; attempt < 30; attempt++) {
        const tx = new Tx(this.backend, this.serial ? null : this.locks);
        try {
          const result = await fn(tx);
          // The service's domain hook sees every transaction's changes before they commit.
          if (this.beforeCommit) await this.beforeCommit(tx);
          const plan = tx.plan();
          // Reads and idle renewals commit nothing, so they never invalidate a concurrent writer.
          if (!plan.writes.length && !plan.events.length) return structuredClone(result);
          if ((await this.backend.commit(plan)).ok) return structuredClone(result);
        } finally { tx.release(); }
        // Exponential backoff with full jitter spreads contenders on other replicas apart.
        await new Promise(r => setTimeout(r, Math.random() * Math.min(250, 5 * 2 ** attempt)));
      }
      throw fail('storage_contention', 'Control storage contention; retry the request', 503);
    };
    if (!this.serial) return run();
    // SQLite has one writer: queue transactions in-process so they never conflict. Never nest transact calls.
    const result = this.tail.then(run, run);
    this.tail = result.catch(() => {});
    return result;
  }
  async beginCommand(id, holder, owner) { return this.backend.beginCommand(id, holder, owner); }
  async finishCommand(id, fence) { return this.backend.finishCommand(id, fence); }
  async close() {
    await this.tail;
    this.backend.close();
  }
}
let singleton;
/**
 * Three backends, one contract. DATABASE_URL wins over Supabase so a deployment
 * can move off it by setting one variable; SQLite is what is left when neither is
 * configured, and it allows exactly one writer.
 */
export function controlStore() {
  return singleton ||= new ControlStore({
    remote: pgRemote() || db,
    path: join(process.env.OYA_DATA_DIR || new URL('../../data/', import.meta.url).pathname, 'control.sqlite'),
  });
}
