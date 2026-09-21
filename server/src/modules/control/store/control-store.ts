/**
 * Transactions and reads over whichever control backend is configured,
 * re-running a transaction when another writer moved a row it read.
 */
import { Status } from '../../../platform/http-status.ts';
import { SqliteBackend } from './sqlite-backend.ts';
import { RemoteBackend } from './remote-backend.ts';
import { KeyedMutex } from './keyed-mutex.ts';
import { Tx } from './tx.ts';
import { fail } from './errors.ts';
import { BACKOFF_GROWTH, BASE_BACKOFF_MS, MAX_BACKOFF_MS, MAX_TX_ATTEMPTS } from './constants.ts';

/** Exponential backoff with full jitter spreads contenders on other replicas apart. */
const backoff = (attempt) =>
  new Promise((r) =>
    setTimeout(r, Math.random() * Math.min(MAX_BACKOFF_MS, BASE_BACKOFF_MS * BACKOFF_GROWTH ** attempt)),
  );

/** Transactions and reads over whichever backend is configured, retried on version conflicts. */
export class ControlStore {
  /** SqliteBackend or RemoteBackend. */
  declare backend: any;
  /** Hook the control service sets to see every transaction before it commits. */
  declare beforeCommit: any;
  /** Row locks for concurrent transactions when the backend is remote. */
  declare locks: KeyedMutex;
  /** True for SQLite: transactions run one at a time in this process. */
  declare serial: any;
  /** The last queued transaction when serial. */
  declare tail: Promise<void>;
  constructor({ path, remote = null, lock = true }: any = {}) {
    this.backend = remote ? new RemoteBackend(remote) : new SqliteBackend(path, { lock });
    this.serial = !remote;
    this.tail = Promise.resolve();
    this.locks = new KeyedMutex();
  }
  /** One row's body by id, or null. */
  async get(kind, id) {
    return (await this.backend.load([{ kind, id }]))[0][0]?.body ?? null;
  }
  /** Bodies matching an indexed filter. */
  async list(kind, filter = {}) {
    return (await this.backend.load([{ kind, ...filter }]))[0].map((r) => r.body);
  }
  /** Rows for several queries in one round trip. */
  async load(queries) {
    return this.backend.load(queries);
  }
  /** Read the event log. */
  async events(options) {
    return this.backend.events(options);
  }
  /** Delete expired rows and events older than each project's cutoff. */
  async prune(now, cutoffs) {
    return this.backend.prune(now, cutoffs);
  }
  /** Run `fn` as a transaction, re-running it on conflict with jittered backoff; results are cloned out. */
  transact(fn) {
    const run = () => this.#retry(fn);
    if (!this.serial) return run();
    // SQLite has one writer: queue transactions in-process so they never conflict. Never nest transact calls.
    const result = this.tail.then(run, run);
    this.tail = result.catch(() => {});
    return result;
  }
  /** Attempt `fn` until it commits, backing off between conflicts. */
  async #retry(fn) {
    for (let attempt = 0; attempt < MAX_TX_ATTEMPTS; attempt++) {
      const committed = await this.#attempt(fn);
      if (committed) return committed.result;
      await backoff(attempt);
    }
    throw fail('storage_contention', 'Control storage contention; retry the request', Status.UNAVAILABLE);
  }
  /** One run of `fn` in a fresh transaction; `{ result }` once committed, null on a version conflict. */
  async #attempt(fn) {
    const tx = new Tx(this.backend, this.serial ? null : this.locks);
    try {
      return await this.#commit(tx, await fn(tx));
    } finally {
      tx.release();
    }
  }
  /** Commit what `tx` changed; `{ result }` (cloned) on success, null on a conflict. */
  async #commit(tx, result) {
    // The service's domain hook sees every transaction's changes before they commit.
    if (this.beforeCommit) await this.beforeCommit(tx);
    const plan = tx.plan();
    // Reads and idle renewals commit nothing, so they never invalidate a concurrent writer.
    const committed = (!plan.writes.length && !plan.events.length) || (await this.backend.commit(plan)).ok;
    return committed ? { result: structuredClone(result) } : null;
  }
  /** Admit an agent command against the session's control gate. */
  async beginCommand(id, holder, owner) {
    return this.backend.beginCommand(id, holder, owner);
  }
  /** Settle a command admitted by beginCommand. */
  async finishCommand(id, fence) {
    return this.backend.finishCommand(id, fence);
  }
  /** Wait for queued transactions, then close the backend. */
  async close() {
    await this.tail;
    this.backend.close();
  }
}
