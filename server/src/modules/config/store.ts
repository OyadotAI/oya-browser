/**
 * The in-memory settings store and its write-behind: every owner's fields,
 * sealing helpers, and a queue that writes the store out without overlapping.
 */
import { db } from '../../platform/db.ts';
import { sealText, openText } from '../../platform/secrets.ts';
import { STORE, writeDb, writeStoreFile } from './repository.ts';

/** owner -> { field: value }. Secret fields hold sealed base64. */
export const store = new Map();
/** Whether the store holds changes not yet written. */
export const state = { dirty: false };
/** Owners whose rows changed since the last write; empty while dirty means every owner. */
export const changed = new Set<string>();

/** Marks one owner's row as changed, so the next write covers it (and only the rows that changed). */
export function markChanged(owner) {
  changed.add(owner);
  state.dirty = true;
}

/**
 * What the next write covers: the owners that changed, or every owner when a
 * change was not attributed to one, with those owners' rows.
 */
export function pendingWrite() {
  const owners = changed.size ? [...changed] : [...store.keys()];
  const rows = owners.flatMap((owner) =>
    Object.entries(store.get(owner) || {}).map(([key, value]) => ({ owner, key, value })),
  );
  return { owners, rows };
}

/** The sealing scope of one owner's values. */
export const scopeFor = (owner) => `key-settings:${owner}`;
/** A value as JSON, sealed for one owner. */
export const seal = (owner, value) => sealText(scopeFor(owner), JSON.stringify(value));
/** A sealed JSON value opened for one owner; throws when it no longer unseals. */
export const unseal = (owner, sealed) => JSON.parse(openText(scopeFor(owner), sealed));

let writeQueue = Promise.resolve();
/** Queue a write so saves never overlap; resolves when this one has run. */
export function flush() {
  writeQueue = writeQueue.catch(() => {}).then(flushOnce);
  return writeQueue;
}

/** Set one field on an owner's row and write the store. */
export async function writeField(owner, field, value) {
  store.set(owner, { ...(store.get(owner) || {}), [field]: value });
  markChanged(owner);
  await flush();
}

/** Remove one field from an owner's row and write the store. */
export async function dropField(owner, field) {
  const row = { ...(store.get(owner) || {}) };
  delete row[field];
  store.set(owner, row);
  markChanged(owner);
  await flush();
}

/**
 * Write the changed owners' settings to key_settings (a save touches one row set,
 * not every tenant's), or the whole store to data/key-settings.json when there
 * is no database or the write fails. A failed file write stays dirty.
 */
async function flushOnce() {
  if (!state.dirty) return;
  const { owners, rows } = pendingWrite();
  state.dirty = false;
  changed.clear();
  if (await wroteDatabase(owners, rows)) return;
  await writeFallback();
}

/** Write to the database if there is one; false means fall back to the file. */
async function wroteDatabase(owners, rows) {
  if (!db) return false;
  try {
    await writeDb(owners, rows);
    return true;
  } catch (e) {
    console.error(`[key-config] database write failed (${e.message}) — falling back to ${STORE}`);
    return false;
  }
}

/** Write the file fallback; on failure the store stays dirty for the next flush. */
async function writeFallback() {
  try {
    await writeStoreFile([...store]);
  } catch (e) {
    state.dirty = true;
    console.error('[key-config] file fallback failed:', e.message);
  }
}
