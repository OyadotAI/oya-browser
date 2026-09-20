/**
 * Recordings as stored: listing, manifests and frames (from the local spool,
 * else the shared archive), deletion, and retention.
 */
import { control } from '../control/service.ts';
import {
  archiveRecording,
  archivedManifest,
  archivedFrame,
  removeArchive,
  sharedRecordings,
} from '../control/recording-storage.ts';
import { readFile, readdir, rm } from 'fs/promises';
import { join } from 'path';
import { DIR, active, frameFile, stop } from './recording-live.ts';
import { DEFAULT_RECORDING_DAYS, MS_PER_DAY, RECORD_MAX_FRAMES, SESSION_ID_PATTERN } from './constants.ts';

/** An entry as a listing shows it: no frame list, and not live. */
const asListed = (m) => ({ ...m, frames: undefined, live: false });

/** Archived recordings, this owner's or every one. */
const archived = (owner) => control().store.list('recording', owner ? { states: [owner] } : {});

/** `owner` null means an operator listing everything. */
export async function list(owner) {
  try {
    const out = await localEntries(owner);
    for (const m of await archived(owner)) if (!out.some((x) => x.sessionId === m.sessionId)) out.push(asListed(m));
    return out.sort((a, b) => String(b.startedAt).localeCompare(String(a.startedAt)));
  } catch {
    return (await archived(owner)).map(asListed);
  }
}

/** Recordings in the local spool, finished or live. */
async function localEntries(owner) {
  const out = [];
  for (const id of await readdir(DIR)) {
    const entry = await localEntry(id, owner);
    if (entry) out.push(entry);
  }
  return out;
}

/** One spooled recording from its manifest, or the live one still being written; null if not theirs. */
async function localEntry(id, owner) {
  try {
    const m = await readManifest(id);
    if (owner && m.owner !== owner) return null;
    return { ...m, frames: undefined, live: active.has(id) };
  } catch {
    return liveSummary(id, owner);
  }
}

/** A live recording with no manifest yet, as a listing shows it; null if there is none or it is not theirs. */
function liveSummary(id, owner) {
  const live = active.get(id);
  if (!live || (owner && live.owner !== owner)) return null;
  return { sessionId: id, owner: live.owner, live: true, frameCount: live.frames.length };
}

/** A spooled recording's manifest; throws when there is none. */
async function readManifest(id) {
  return JSON.parse(await readFile(join(DIR, id, 'manifest.json'), 'utf8'));
}

/** Returns null rather than 403 for someone else's recording: its existence is not their business. */
export async function manifest(sessionId, owner) {
  if (!SESSION_ID_PATTERN.test(sessionId)) return null;
  const live = active.get(sessionId);
  if (live) return owner && live.owner !== owner ? null : liveManifest(sessionId, live);
  return storedManifest(sessionId, owner);
}

/** The manifest so far of a recording still running. */
function liveManifest(sessionId, live) {
  return { sessionId, owner: live.owner, live: true, ...liveContents(live) };
}

/** A running recording's frames and duration so far. */
function liveContents(live) {
  return {
    frameCount: live.frames.length,
    bytes: live.bytes,
    durationMs: Date.now() - live.startedAt,
    frames: live.frames,
  };
}

/** A finished recording's manifest, from the spool else the archive. */
async function storedManifest(sessionId, owner) {
  try {
    const m = await readManifest(sessionId);
    if (owner && m.owner !== owner) return null;
    return m;
  } catch {
    return archivedManifest(sessionId, owner);
  }
}

/** A frame index in range for a well-formed session id. */
const validFrame = (sessionId, i) =>
  Number.isInteger(i) && i >= 0 && i <= RECORD_MAX_FRAMES && SESSION_ID_PATTERN.test(sessionId);

/** One JPEG frame, subject to the same ownership check as the manifest; null if absent or not theirs. */
export async function frame(sessionId, index, owner) {
  const i = Number(index);
  if (!validFrame(sessionId, i)) return null;
  // A frame is a screenshot of a browser, so the same check as the manifest.
  if (!(await manifest(sessionId, owner))) return null;
  try {
    return await readFile(join(DIR, sessionId, frameFile(i)));
  } catch {
    return archivedFrame(sessionId, i, owner);
  }
}

/** Delete a recording, stopping it first if live. False if absent or not theirs. */
export async function remove(sessionId, owner) {
  if (!SESSION_ID_PATTERN.test(sessionId)) return false;
  if (!(await manifest(sessionId, owner))) return false;
  if (active.has(sessionId)) await stop(sessionId);
  await removeArchive(sessionId, owner);
  return removeSpool(sessionId);
}

/** Deletes the local spool; false if that failed. */
async function removeSpool(sessionId) {
  try {
    await rm(join(DIR, sessionId), { recursive: true, force: true });
    return true;
  } catch {
    return false;
  }
}

/** Retention and failed-upload retry run independently of browser command traffic. */
export async function maintain() {
  const [projects, stored] = await control().store.load([{ kind: 'project' }, { kind: 'recording' }]);
  const days = new Map<string, number>(projects.map(({ body: p }) => [p.legacyOwner, p.settings.recordingDays]));
  const archivedIds = new Set(stored.map((r) => r.id));
  for (const entry of await list(null)) if (!entry.live) await maintainOne(entry, days, archivedIds);
}

/** Deletes an expired recording, or retries archiving one the archive does not have yet. */
async function maintainOne(entry, days, archivedIds) {
  if (Date.parse(entry.startedAt) < Date.now() - (days.get(entry.owner) || DEFAULT_RECORDING_DAYS) * MS_PER_DAY) {
    await remove(entry.sessionId, entry.owner);
  } else if (sharedRecordings() && !archivedIds.has(entry.sessionId)) {
    const m = await manifest(entry.sessionId, entry.owner);
    if (m) await archiveRecording(m, join(DIR, entry.sessionId));
  }
}
