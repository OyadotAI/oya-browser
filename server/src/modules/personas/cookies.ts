/** Durable, persona-scoped login state. Cookies and localStorage share one store. */
import { readFileSync, renameSync } from 'fs';
import { mkdir, writeFile, rename } from 'fs/promises';
import { dirname } from 'path';
import { defaultPersonaSeed } from './fingerprint.ts';
import { sealText, openText } from '../../platform/secrets.ts';
import { dataPath } from '../../platform/paths.ts';
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
const FILE = dataPath('cookies.json');
/** persona id -> (cookie key -> cookie). */
const jars = new Map();
/** persona id -> localStorage by origin. */
const origins = new Map();
/** persona id -> ISO time its state last changed. */
const updated = new Map();
/** Bumped on every change; the save catches up to it. */
let revision = 0;
/** The revision last written to disk. */
let savedRevision = 0;
/** The pending save, if one is scheduled. */
let timer = null;
/** The write in flight, if any. */
let writing = null;

/** Mark a persona's state as changed and schedule a save half a second out; a failed save retries. */
function changed(id) {
  updated.set(id, new Date().toISOString());
  revision++;
  if (!timer) scheduleSave(id);
}

/** Saves after the batching delay; a failure marks `id` changed again so it retries. */
function scheduleSave(id) {
  timer = setTimeout(() => {
    timer = null;
    flush().catch((e) => {
      console.error('[profiles] Save failed:', e.message);
      changed(id);
    });
  }, LOGIN_SAVE_DELAY_MS);
  timer.unref?.();
}

/** Serialize writers and rename atomically; never truncate a working profile. */
async function flush() {
  if (writing) return flushAfterWrite();
  if (savedRevision === revision) return;
  writing = writeRecords(sealAll(), revision);
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

/** Every persona's state, sealed under its own scope. */
function sealAll() {
  const records = {};
  for (const id of new Set([...jars.keys(), ...origins.keys()])) records[id] = sealText(`login:${id}`, stateOf(id));
  return records;
}

/** One persona's saved state: unexpired cookies, localStorage and when it changed. */
const stateOf = (id) => ({ cookies: getAll(id), origins: getStorage(id), updatedAt: updated.get(id) || null });

/** Writes the sealed records through a temp file and a rename, then records the revision saved. */
async function writeRecords(records, snapshotRevision) {
  await mkdir(dirname(FILE), { recursive: true, mode: 0o700 });
  const temp = `${FILE}.${process.pid}.tmp`;
  await writeFile(temp, JSON.stringify({ version: LOGIN_FILE_VERSION, records }), { mode: 0o600 });
  await rename(temp, FILE);
  savedRevision = snapshotRevision;
}

/** Write out every pending change now, e.g. before shutdown. */
export async function drain() {
  clearTimeout(timer);
  timer = null;
  while (savedRevision !== revision) await flush();
}

// Load before accepting browsers. Migrate owned jars; never guess an unscoped jar's owner.
try {
  load(JSON.parse(readFileSync(FILE, 'utf8')));
} catch (e) {
  if (e.code !== 'ENOENT') throw new Error(`Cannot read saved login state: ${e.message}`);
}

/** Loads the saved file: sealed records, or a legacy dump to migrate or set aside. */
function load(data) {
  if (Array.isArray(data)) {
    renameSync(FILE, `${FILE}.legacy-${Date.now()}`);
  } else if (data.version === LOGIN_FILE_VERSION) {
    for (const [id, sealed] of Object.entries(data.records || {})) loadSealed(id, sealed);
  } else {
    loadUnscoped(data);
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

/** Merge a full cookie dump into the persona's jar; expired ones remove their match. Returns the unexpired cookies. */
export function mergeDump(id, cookies) {
  if (!id || !Array.isArray(cookies)) return [];
  const jar = jarOf(id);
  for (const c of cookies.filter(validCookie)) {
    if (expired(c)) jar.delete(cookieKey(c));
    else jar.set(cookieKey(c), c);
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
  else jar.set(cookieKey(c), c);
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
