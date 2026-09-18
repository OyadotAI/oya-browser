/**
 * One optimistic control-store transaction: reads are cached as they happen,
 * changes are buffered, and plan() turns them into version-checked writes.
 */
import { columns, matches } from './columns.ts';

/** A row's body as stored: sessions drop the in-flight count, which lives in their gate. */
function storedBody(r) {
  if (!r.body || r.kind !== 'session') return r.body;
  const { inFlight, ...stored } = r.body;
  return stored;
}

/** Whether a cached row has to be written: changed or locked, and not a row that never existed and still does not. */
const needsWrite = (r) => !((JSON.stringify(r.body) === r.original && !r.locked) || (!r.body && !r.version));

/** The version-checked write for a cached row. */
function toWrite(r) {
  const body = storedBody(r);
  return { kind: r.kind, id: r.id, version: r.version, body, ...(body ? columns(r.kind, body) : {}) };
}

/**
 * One optimistic transaction: reads are cached as they happen, changes are buffered, and plan() turns them
 * into version-checked writes. Discarded and re-run on conflict.
 */
export class Tx {
  /** Row locks held in this process, released after the commit attempt. */
  declare releases: Map<any, any>;
  /** Backend the transaction reads from. */
  declare backend: any;
  /** Events emitted during the transaction, committed with its writes. */
  declare events: any;
  /** Filters already listed, so list() serves them from the cache. */
  declare fetched: Set<any>;
  /** In-process mutex for row locks; null when the store is already serial (SQLite). */
  declare mutex: any;
  /** Every row read or written, keyed by kind and id, with its version and body as read. */
  declare rows: Map<any, any>;
  constructor(backend, mutex = null) {
    this.backend = backend;
    this.mutex = mutex;
    this.rows = new Map();
    this.events = [];
    this.releases = new Map();
    this.fetched = new Set();
  }
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
  /** Cache a loaded row once; later loads never overwrite what this transaction already holds. */
  #entry(kind, row) {
    const { id, version, body } = row,
      key = `${kind} ${id}`;
    if (!this.rows.has(key)) this.rows.set(key, { kind, id, version, original: JSON.stringify(body), body });
    return this.rows.get(key);
  }
  /** One row by id, or null. */
  async get(kind, id) {
    return (await this.getMany(kind, [id]))[0] ?? null;
  }
  /** One round trip for several rows; missing rows are omitted. */
  async getMany(kind, ids) {
    const missing = [...new Set(ids)].filter((id) => !this.rows.has(`${kind} ${id}`));
    if (missing.length) {
      const found = await this.backend.load(missing.map((id) => ({ kind, id })));
      missing.forEach((id, i) => this.#entry(kind, found[i][0] || { id, version: 0, body: null }));
    }
    return ids.map((id) => this.rows.get(`${kind} ${id}`).body).filter(Boolean);
  }
  /** Rows matching an indexed filter, merged with this transaction's own changes. */
  async list(kind, filter = {}) {
    if (!this.fetched.has(JSON.stringify([kind, filter]))) {
      const [found] = await this.backend.load([{ kind, ...filter }]);
      for (const row of found) this.#entry(kind, row);
    }
    return [...this.rows.values()]
      .filter((r) => r.kind === kind && r.body && matches(kind, r.body, filter))
      .map((r) => r.body);
  }
  /** Insert a new row or replace one this transaction has read. */
  put(kind, id, body) {
    const entry = this.rows.get(`${kind} ${id}`);
    if (entry) entry.body = body;
    else this.rows.set(`${kind} ${id}`, { kind, id, version: 0, original: 'null', body });
    return body;
  }
  /** Mark a row deleted; the delete is written at commit. */
  async delete(kind, id) {
    await this.get(kind, id);
    this.rows.get(`${kind} ${id}`).body = null;
  }
  /**
   * Serialize with every other transaction that locks the same row, creating it if needed. Lockers in this
   * process queue on a mutex held until commit, so under a burst only other replicas can conflict.
   */
  async lock(kind, id) {
    const key = [kind, id].join(' ');
    if (this.mutex && !this.releases.has(key)) await this.#lockLocally(kind, id, key);
    if (!(await this.get(kind, id))) this.put(kind, id, { id });
    this.rows.get(key).locked = true;
  }
  /** Queue on this process's mutex for the row, holding it until commit. */
  async #lockLocally(kind, id, key) {
    const { release, waited } = await this.mutex.acquire(key);
    this.releases.set(key, release);
    const cached = this.rows.get(key);
    // A local transaction may have committed this row while we waited; lock on the version it left.
    if (waited && cached?.version) await this.#refresh(kind, id, cached);
  }
  /** Replace a cached row, in place, with its latest committed version. */
  async #refresh(kind, id, cached) {
    const [[fresh]] = await this.backend.load([{ kind, id }]);
    if (!fresh || fresh.version === cached.version) return;
    for (const field of Object.keys(cached.body)) delete cached.body[field];
    Object.assign(cached.body, fresh.body);
    Object.assign(cached, { version: fresh.version, original: JSON.stringify(fresh.body) });
  }
  /** Let other in-process transactions waiting on this one's row locks go ahead. */
  release() {
    for (const release of this.releases.values()) release();
    this.releases.clear();
  }
  /** Rows of a kind this transaction changed, with their state as read. */
  changes(kind) {
    return [...this.rows.values()]
      .filter((r) => r.kind === kind && r.body && JSON.stringify(r.body) !== r.original)
      .map((r) => ({ before: JSON.parse(r.original), after: r.body }));
  }
  /** Record an event to commit with the transaction and bump its session's updatedAt. */
  emit(project, type, sessionId = null, detail = {}) {
    const at = Date.now();
    this.events.push({ project, type, sessionId, at, detail });
    const session = sessionId && this.rows.get(`session ${sessionId}`)?.body;
    if (session) session.updatedAt = at;
  }
  /** The writes (changed or locked rows, minus sessions' in-flight counts) and events to commit. */
  plan() {
    return { writes: [...this.rows.values()].filter(needsWrite).map(toWrite), events: this.events };
  }
}
