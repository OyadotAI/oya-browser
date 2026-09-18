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
  state.dirty = true;
  await flush();
}

/** Remove one field from an owner's row and write the store. */
export async function dropField(owner, field) {
  const row = { ...(store.get(owner) || {}) };
  delete row[field];
  store.set(owner, row);
  state.dirty = true;
  await flush();
}

/**
 * Write every key's settings to key_settings, or to data/key-settings.json when
 * there is no database or the write fails. A failed file write stays dirty.
 */
async function flushOnce() {
  if (!state.dirty) return;
  state.dirty = false;
  const rows = [...store.entries()].flatMap(([owner, fields]) =>
    Object.entries(fields).map(([key, value]) => ({ owner, key, value })),
  );
  if (await wroteDatabase(rows)) return;
  await writeFallback();
}

/** Write to the database if there is one; false means fall back to the file. */
async function wroteDatabase(rows) {
  if (!db) return false;
  try {
    await writeDb([...store.keys()], rows);
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
