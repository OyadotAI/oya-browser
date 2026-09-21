/**
 * Persistent browser profiles.
 *
 * Cookies and origin storage are captured when a gateway session ends and
 * replayed when the next session names the same profile, so a login survives
 * across sessions and across providers.
 *
 * At rest: AES-256-GCM under a per-profile data key, itself wrapped by a KEK
 * derived from OYA_PROFILE_SECRET with scrypt. The profile name is the AAD on
 * both layers, so a ciphertext moved onto another profile fails to open rather
 * than silently handing one tenant's session to another.
 */

import { timingSafeEqual } from 'crypto';
import { seal as sealScoped, open as openScoped } from '../../platform/secrets.ts';
import { readFile, writeFile, mkdir, readdir, unlink } from 'fs/promises';
import { join } from 'path';
import { dataPath } from '../../platform/paths.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { openPage } from './cdp-page.ts';
import { ORIGIN_STORAGE_JS, restoreStorageJS } from './profile-storage-js.ts';
import { PRIVATE_DIR_MODE, PRIVATE_FILE_MODE, PROFILE_SUFFIX } from './constants.ts';

/** Where sealed profiles are kept. */
const DIR = dataPath('profiles');

/** The name, refused with a 400 unless it is 1-64 safe characters. */
const safeName = (name) => {
  if (typeof name !== 'string' || !/^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/.test(name)) {
    throw new HttpError(Status.BAD_REQUEST, 'Profile names are 1-64 chars of letters, digits, dot, dash or underscore');
  }
  return name;
};

/**
 * Profiles are namespaced by owner. Without this, naming another tenant's
 * profile on connect would hand over their cookies and localStorage, the
 * names are chosen by callers and are not secrets.
 */
const safeOwner = (owner) => {
  if (typeof owner !== 'string' || !/^[0-9a-f]{8,64}$/.test(owner)) {
    throw new HttpError(Status.BAD_REQUEST, 'A profile owner fingerprint is required');
  }
  return owner;
};

/** The owner-namespaced key a profile is locked and sealed under. */
const scopeOf = (owner, name) => `${safeOwner(owner)}__${safeName(name)}`;
/** The file a profile is saved in. */
const fileFor = (owner, name) => join(DIR, `${scopeOf(owner, name)}${PROFILE_SUFFIX}`);

/** Seals a profile payload under its scope. */
const seal = (scope, value) => sealScoped(`profile:${scope}`, value);

/** Opens a sealed profile; fails under any other scope. */
const open = (scope, buf) => openScoped(`profile:${scope}`, buf);

// ── Concurrency lock ──
// One writer per profile. Two sessions sharing a jar interleave writes and
// corrupt it, so the second connect is refused rather than allowed to race.
/** scope -> { owner, since } */
const locks = new Map();

/** Claim the profile for one session; `{ ok: false, since }` if another session already holds it. */
export function tryLock(owner, name) {
  const scope = scopeOf(owner, name);
  const held = locks.get(scope);
  if (held) return { ok: false, since: held.since };
  locks.set(scope, { owner, since: Date.now() });
  return { ok: true };
}

/** Release the profile's session lock. */
export function unlock(owner, name) {
  locks.delete(scopeOf(owner, name));
}
/** Whether a session currently holds the profile. */
export function isLocked(owner, name) {
  return locks.has(scopeOf(owner, name));
}

/** A second CDP connection, so the client's own wire is never touched. */
async function attach(session) {
  const attached = await openPage(session.upstreamUrl);
  if (!attached) return null;
  await attached.conn.send('Network.enable', {}, attached.sessionId).catch(() => {});
  await attached.conn.send('Runtime.enable', {}, attached.sessionId).catch(() => {});
  return attached;
}

/** Read the live browser state and persist it under this profile. */
export async function capture(owner, name, session) {
  const scope = scopeOf(owner, name);
  const attached = await attach(session);
  if (!attached) return false;
  return saveFrom(owner, name, scope, attached);
}

/** Snapshots the attached page into the profile file, closing the connection either way. */
async function saveFrom(owner, name, scope, { conn, sessionId }) {
  try {
    await persist(owner, name, scope, await snapshot(conn, sessionId));
    return true;
  } finally {
    conn.close();
  }
}

/** The browser's cookies and its current origin's storage. */
async function snapshot(conn, sessionId) {
  const { cookies = [] } = await conn.send('Network.getAllCookies', {}, sessionId);
  const storage = await originStorage(conn, sessionId);
  return { version: 1, savedAt: new Date().toISOString(), cookies, storage: storage?.result?.value || null };
}

/** The current origin's storage; null when the page will not say. */
async function originStorage(conn, sessionId) {
  try {
    return await conn.send('Runtime.evaluate', { expression: ORIGIN_STORAGE_JS, returnByValue: true }, sessionId);
  } catch {
    return null;
  }
}

/** Seals the payload and writes it, owner-only. */
async function persist(owner, name, scope, payload) {
  await mkdir(DIR, { recursive: true, mode: PRIVATE_DIR_MODE });
  await writeFile(fileFor(owner, name), seal(scope, payload), { mode: PRIVATE_FILE_MODE });
}

/** Replay a stored profile into a fresh browser before the client uses it. */
export async function restore(owner, name, session) {
  const scope = scopeOf(owner, name);
  const payload = await load(owner, name, scope);
  if (payload === null) return false; // first use of this profile
  const attached = await attach(session);
  if (!attached) return false;
  await replay(payload, attached, session);
  return true;
}

/** The saved profile, or null when there is none yet. */
async function load(owner, name, scope) {
  try {
    return open(scope, await readFile(fileFor(owner, name)));
  } catch (e) {
    if (e.code === 'ENOENT') return null;
    throw e;
  }
}

/** Sets the cookies, and registers the storage replay when there is storage to replay. */
async function replay(payload, { conn, sessionId }, session) {
  if (payload.cookies?.length) {
    await conn.send('Network.setCookies', { cookies: payload.cookies }, sessionId);
  }
  if (!payload.storage?.origin) return conn.close();
  await replayStorage(conn, sessionId, payload.storage);
  session.profileConn = conn;
}

/**
 * Storage is origin-scoped, so it can only be written once a document on
 * that origin exists. addScriptToEvaluateOnNewDocument is scoped to the
 * connection that registered it, so this connection has to outlive the
 * call, it is closed when the session ends.
 */
async function replayStorage(conn, sessionId, storage) {
  await conn.send('Page.enable', {}, sessionId).catch(() => {});
  const source = `if (location.origin === ${JSON.stringify(storage.origin)}) ${restoreStorageJS(storage)};`;
  await conn.send('Page.addScriptToEvaluateOnNewDocument', { source }, sessionId).catch(() => {});
}

/** Only this owner's profiles. Names are caller-chosen and must not leak. */
export async function list(owner) {
  const prefix = `${safeOwner(owner)}__`;
  try {
    const files = await readdir(DIR);
    return files.filter((f) => f.endsWith(PROFILE_SUFFIX) && f.startsWith(prefix)).map((f) => describe(prefix, f));
  } catch {
    return [];
  }
}

/** A saved profile's name and whether a session holds it. */
function describe(prefix, file) {
  const name = file.slice(prefix.length, -PROFILE_SUFFIX.length);
  const scope = `${prefix}${name}`;
  return { name, locked: locks.has(scope), lockedSince: locks.get(scope)?.since || null };
}

/** Delete a saved profile; refused (409) while a session holds it, false if there was nothing to delete. */
export async function remove(owner, name) {
  if (isLocked(owner, name)) throw new HttpError(Status.CONFLICT, 'Profile is in use');
  try {
    await unlink(fileFor(owner, name));
    return true;
  } catch {
    return false;
  }
}

/** Exposed for tests: prove a profile cannot be opened under another name. */
export const _internals = { seal, open, timingSafeEqual };
