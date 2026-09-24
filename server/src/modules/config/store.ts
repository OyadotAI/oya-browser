/**
 * The in-memory settings store and its write-behind: every owner's fields,
 * sealing helpers, and a queue that writes the store out without overlapping.
 */
import { sealText, openText } from '../../platform/secrets.ts';
import { writeRows } from './repository.ts';

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
 * not every tenant's). A failed write puts those owners back, so the next flush
 * retries them, and says so.
 */
async function flushOnce() {
  if (!state.dirty) return;
  const { owners, rows } = pendingWrite();
  state.dirty = false;
  changed.clear();
  await writeRows(owners, rows).catch((e) => requeue(owners, e));
}

/** Puts owners whose write failed back in the queue, and says so. */
function requeue(owners, e) {
  for (const owner of owners) changed.add(owner);
  state.dirty = true;
  console.error(`[key-config] write failed (${e.message}); the next save retries it`);
}
