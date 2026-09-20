/**
 * Loading the settings store at startup and waiting for its writes at shutdown.
 */
import { db } from '../../platform/db.ts';
import { STORE, readDb, readStoreFile } from './repository.ts';
import { store, flush } from './store.ts';
import { cleanupPlaybooks } from './playbooks.ts';

/** Merge database rows into the store. */
function loadRows(rows) {
  for (const row of rows) {
    const cur = store.get(row.owner) || {};
    cur[row.key] = row.value;
    store.set(row.owner, cur);
  }
}

/** Read every row from the database, then upgrade saved playbooks. */
async function loadDatabase() {
  loadRows(await readDb());
  if (store.size) console.log(`[key-config] restored settings for ${store.size} keys`);
  await cleanupPlaybooks();
}

/** Load from the database if there is one; false means fall back to the file. */
async function restoredDatabase() {
  if (!db) return false;
  try {
    await loadDatabase();
    return true;
  } catch (e) {
    console.error(`[key-config] database read failed (${e.message}) — falling back to ${STORE}`);
    return false;
  }
}

/** Load the file fallback; a missing file is a fresh install. */
async function restoreFile() {
  try {
    for (const [owner, fields] of await readStoreFile()) store.set(owner, fields);
  } catch (e) {
    if (e.code !== 'ENOENT') console.error('[key-config] restore failed:', e.message);
  }
}

/** Load settings from the database, else the file, then upgrade saved playbooks. */
export async function restore() {
  if (await restoredDatabase()) return;
  await restoreFile();
  await cleanupPlaybooks();
}

/** Wait for pending writes, for shutdown. */
export async function drain() {
  await flush();
}
