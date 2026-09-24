/**
 * Loading the settings store at startup and waiting for its writes at shutdown.
 */
import { readRows } from './repository.ts';
import { store, flush } from './store.ts';
import { cleanupPlaybooks } from './playbooks.ts';

/** Merge stored rows into the store. */
function loadRows(rows) {
  for (const row of rows) {
    const cur = store.get(row.owner) || {};
    cur[row.key] = row.value;
    store.set(row.owner, cur);
  }
}

/** Load settings from storage, then upgrade saved playbooks. A storage that cannot be read fails the start. */
export async function restore() {
  loadRows(await readRows());
  if (store.size) console.log(`[key-config] restored settings for ${store.size} keys`);
  await cleanupPlaybooks();
}

/** Wait for pending writes, for shutdown. */
export async function drain() {
  await flush();
}
