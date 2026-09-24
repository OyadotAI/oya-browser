/** Durable, persona-scoped login state. Cookies and localStorage share one store. */
import { defaultPersonaSeed } from './fingerprint.ts';
import { sealText, openText } from '../../platform/secrets.ts';
import { dataPath } from '../../platform/paths.ts';
import { RecordTable, importLegacyFile } from '../../platform/storage/index.ts';
import {
  cookieKey,
  validCookie,
  expired,
  isRecord,
  hostsOf,
  carries,
  validOrigin,
  storageEntries,
} from './cookie-rules.ts';
import { LOGIN_FILE_VERSION, LOGIN_SAVE_DELAY_MS, MAX_STORAGE_ORIGINS } from './constants.ts';

/** Where login state is kept, sealed per persona. */
const table = new RecordTable('persona_logins');
/** The file it lived in before storage drivers, imported once. */
const LEGACY_FILE = dataPath('cookies.json');
/** persona id -> (cookie key -> cookie). */
const jars = new Map();
/** persona id -> localStorage by origin. */
const origins = new Map();
/** persona id -> ISO time its state last changed. */
const updated = new Map();
/** Personas whose state changed since it was last written. */
const dirty = new Set<string>();
/** Bumped on every change; the save catches up to it. */
let revision = 0;
/** The revision last written. */
let savedRevision = 0;
/** The pending save, if one is scheduled. */
let timer = null;
/** The write in flight, if any. */
let writing = null;

/** Mark a persona's state as changed and schedule a save half a second out. */
function changed(id) {
  updated.set(id, new Date().toISOString());
  markDirty(id);
}

/** Queue a persona's state for the next save, without touching when it changed. */
function markDirty(id) {
  dirty.add(id);
  revision++;
  if (!timer) scheduleSave();
}

/** Saves after the batching delay; a failed save keeps its personas queued and tries again. */
function scheduleSave() {
  timer = setTimeout(() => {
    timer = null;
    flush().catch((e) => console.error('[profiles] Save failed:', e.message));
  }, LOGIN_SAVE_DELAY_MS);
  timer.unref?.();
}

/** Serialize writers; one write at a time, each covering what changed before it began. */
async function flush() {
  if (writing) return flushAfterWrite();
  if (savedRevision === revision) return;
  writing = writeRecords(revision);
  try {
    await writing;
  } finally {
    writing = null;
  }
}

/** Waits out the write in flight, then flushes whatever it missed. */
async function flushAfterWrite() {
  await writing;
  return flush();
}

/** Writes the changed personas' sealed state and removes cleared ones; on failure they stay queued. */
async function writeRecords(snapshotRevision) {
  const ids = [...dirty];
  dirty.clear();
  await saveIds(ids).catch((e) => {
    for (const id of ids) markDirty(id);
    throw e;
  });
  savedRevision = snapshotRevision;
}

/** Each id's state sealed under its own scope, or removed when the persona holds none any more. */
async function saveIds(ids: string[]) {
  const kept = ids.filter((id) => jars.has(id) || origins.has(id));
  await table.put(kept.map((id) => [id, sealText(`login:${id}`, stateOf(id))]));
  await table.remove(ids.filter((id) => !kept.includes(id)));
}

/** One persona's saved state: unexpired cookies, localStorage and when it changed. */
const stateOf = (id) => ({ cookies: getAll(id), origins: getStorage(id), updatedAt: updated.get(id) || null });

/** Write out every pending change now, e.g. before shutdown. */
export async function drain() {
  clearTimeout(timer);
  timer = null;
  while (savedRevision !== revision) await flush();
}

/** Load before accepting browsers: the legacy file first if it is still there, then every stored persona. */
export async function restore() {
  await importLegacyFile(LEGACY_FILE, async (data) => {
    load(data);
    await drain();
  });
  for (const [id, sealed] of await table.load()) loadSealed(id, sealed);
}

/**
 * Takes in the legacy file: sealed records, or the old key-per-jar dump to
 * migrate. A bare array came from before jars had owners; never guess one, so
 * it is only set aside.
 */
function load(data) {
  if (Array.isArray(data)) return;
  if (data.version !== LOGIN_FILE_VERSION) return loadUnscoped(data);
  for (const [id, sealed] of Object.entries(data.records || {})) {
    loadSealed(id, sealed);
    markDirty(id);
  }
}

/** Opens one persona's sealed state into memory. */
function loadSealed(id, sealed) {
  const state = openText(`login:${id}`, sealed);
  jars.set(id, new Map((state.cookies || []).filter(validCookie).map((c) => [cookieKey(c), c])));
  origins.set(id, state.origins || {});
  updated.set(id, state.updatedAt || null);
}

/** Migrates the old key-per-jar file: persona ids stay, API keys map to their default persona. */
function loadUnscoped(data) {
  for (const [key, cookies] of Object.entries(data)) {
    if (!Array.isArray(cookies)) continue;
    const id = /^(apikey-[0-9a-f]{12}|p-[0-9a-f]{16})$/.test(key) ? key : defaultPersonaSeed(key).id;
    mergeDump(id, cookies);
  }
}

/** The persona's jar, created empty on first use. */
function jarOf(id) {
  if (!jars.has(id)) jars.set(id, new Map());
  return jars.get(id);
}

/** A cookie as the browser sent it: without the stamp this store adds. */
const unstamped = ({ t: _t, ...cookie }) => JSON.stringify(cookie);

/**
 * Keeps a cookie, stamped with when it last really changed. A dump repeats the
 * whole jar on every connect, so a repeat keeps its stamp: the stamp is how a
 * browser tells a cookie another browser refreshed since it last synced from
 * one it already has, and stale copies stopped overwriting fresh logins.
 */
function keep(jar, c) {
  const key = cookieKey(c);
  const old = jar.get(key);
  jar.set(key, { ...c, t: old && unstamped(old) === unstamped(c) ? old.t : Date.now() });
}

/** Merge a full cookie dump into the persona's jar; expired ones remove their match. Returns the unexpired cookies. */
export function mergeDump(id, cookies) {
  if (!id || !Array.isArray(cookies)) return [];
  const jar = jarOf(id);
  for (const c of cookies.filter(validCookie)) {
    if (expired(c)) jar.delete(cookieKey(c));
    else keep(jar, c);
  }
  changed(id);
  return getAll(id);
}

/** Apply one cookie added or removed in the browser. */
export function applyChange(id, change) {
  if (!id || !validCookie(change?.cookie)) return null;
  const jar = jarOf(id),
    c = change.cookie;
  if (change.removed || expired(c)) jar.delete(cookieKey(c));
  else keep(jar, c);
  changed(id);
  return change;
}

/** The persona's unexpired cookies. */
export function getAll(id) {
  return [...(jars.get(id)?.values() || [])].filter((c) => !expired(c));
}

/** The persona's cookies that a request to any of these hosts would carry (at most 20 hosts). */
export function getForDomains(id, domains) {
  if (!Array.isArray(domains)) return [];
  const hosts = hostsOf(domains);
  return getAll(id).filter((c) => carries(c, hosts));
}

/** The persona's localStorage, by origin. */
export function getStorage(id) {
  return origins.get(id) || {};
}

/** Replace only visited origins, including empty storage after logout. */
export function mergeStorage(id, values) {
  if (!id || !isRecord(values)) return;
  const current = { ...getStorage(id) };
  for (const [origin, items] of Object.entries(values).slice(0, MAX_STORAGE_ORIGINS)) {
    const entries = validOrigin(origin) && storageEntries(items);
    if (entries) current[origin] = Object.fromEntries(entries);
  }
  origins.set(id, current);
  changed(id);
}

/** Metadata only; credential values never belong in a list. */
export function summary(id) {
  const cookies = getAll(id);
  const sites = new Set(cookies.map((c) => c.domain.replace(/^\./, '')));
  for (const [origin, items] of Object.entries(getStorage(id)))
    if (Object.keys(items).length) sites.add(new URL(origin).hostname);
  return { cookies: cookies.length, sites: [...sites].sort(), updatedAt: updated.get(id) || null };
}

/** Forget the persona's cookies and storage. */
export function clear(id) {
  if (!id) return;
  jars.delete(id);
  origins.delete(id);
  updated.delete(id);
  changed(id);
}
