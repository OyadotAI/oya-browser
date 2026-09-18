/**
 * Row-level control storage. Every entity is a versioned row; a transaction writes only the rows it
 * changed, with compare-and-swap on each row's version. Events are an append-only log, and webhook
 * deliveries are created atomically with the events that trigger them.
 *
 * Transactions may await their own reads (tx.get/list) but must perform no other I/O: they are re-run on conflict.
 * Invariants that span rows (capacity, provider holds) take a lock row: tx.lock bumps its version, so concurrent
 * transactions that decide from the same set of rows serialize on it.
 *
 * This is the entry point; the backends, transaction and retry loop live in store/.
 */
import { db } from '../../platform/db.ts';
import { pgRemote } from './pg-client.ts';
import { dataPath } from '../../platform/paths.ts';
import { ControlStore } from './store/control-store.ts';

export { columns } from './store/columns.ts';
export { SqliteBackend } from './store/sqlite-backend.ts';
export { RemoteBackend } from './store/remote-backend.ts';
export { Tx } from './store/tx.ts';
export { ControlStore };

let singleton;
/**
 * Three backends, one contract. DATABASE_URL wins over Supabase so a deployment
 * can move off it by setting one variable; SQLite is what is left when neither is
 * configured, and it allows exactly one writer.
 */
export function controlStore() {
  return (singleton ||= new ControlStore({
    remote: pgRemote() || db,
    path: dataPath('control.sqlite'),
  }));
}
