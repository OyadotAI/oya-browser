/** Private object storage for recordings, with a durable local spool on upload failure. */
import { readFile, readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { control } from './service.ts';
import { DELETE_BATCH } from './recording-storage/constants.ts';
import {
  archived,
  assertPrivateBucket,
  assertShared,
  frameName,
  objects,
  safe,
  sharedRecordings,
} from './recording-storage/bucket.ts';

export { sharedRecordings };

/** Upload one file of a recording directory under `prefix`. */
async function upload(prefix, directory, name) {
  const { error } = await objects().upload(`${prefix}/${name}`, await readFile(join(directory, name)), {
    upsert: true,
    contentType: name.endsWith('.jpg') ? 'image/jpeg' : 'application/json',
  });
  if (error) throw new Error('Recording archive upload failed');
}

/** Uploads a finished recording's frames and manifest to the private bucket and marks it archived. Refuses a public bucket. */
export async function archiveRecording(manifest, directory) {
  if (!sharedRecordings()) return;
  await assertPrivateBucket();
  const prefix = `${manifest.owner}/${manifest.sessionId}`;
  for (const name of await readdir(directory)) if (archived(name)) await upload(prefix, directory, name);
  await control().store.transact(async (tx) => {
    await tx.get('recording', manifest.sessionId);
    tx.put('recording', manifest.sessionId, { ...manifest, frames: undefined, archived: true });
  });
}

/** The recording's record, or null when the id is unknown or belongs to another owner. */
async function ownedRecord(id, owner) {
  const m = await control().store.get('recording', id);
  return !m || (owner && m.owner !== owner) ? null : m;
}

/** An archived recording's manifest, or null when the id is unknown or belongs to another owner. */
export async function archivedManifest(id, owner) {
  if (!safe(id)) return null;
  const m = await ownedRecord(id, owner);
  if (!m) return null;
  assertShared();
  const { data, error } = await objects().download(`${m.owner}/${id}/manifest.json`);
  if (error) throw new Error('Recording manifest unavailable');
  return JSON.parse(await data.text());
}

/** One archived JPEG frame, or null when it is missing or not the caller's. */
export async function archivedFrame(id, index, owner) {
  const m = await archivedManifest(id, owner);
  if (!m || !sharedRecordings()) return null;
  const { data, error } = await objects().download(`${m.owner}/${id}/${frameName(index)}`);
  return error ? null : Buffer.from(await data.arrayBuffer());
}

/** Every frame path of an archived recording, persisted on its record before anything is deleted; null when it has no manifest. */
async function planDeletion(id, owner) {
  const m = await archivedManifest(id, owner);
  if (!m) return null;
  const paths = m.frames.map((f) => `${m.owner}/${id}/${frameName(f.i)}`);
  // Persist the deletion plan before deleting the manifest so a crash can resume.
  await control().store.transact(async (tx) => {
    const r = await tx.get('recording', id);
    if (r) r.deletePaths = paths;
  });
  return paths;
}

/** Remove storage paths in batches. */
async function removeAll(paths) {
  for (let i = 0; i < paths.length; i += DELETE_BATCH) {
    const { error } = await objects().remove(paths.slice(i, i + DELETE_BATCH));
    if (error) throw new Error('Recording archive deletion failed');
  }
}

/** Remove the manifest, then the record. */
async function removeManifest(entry, id) {
  const { error } = await objects().remove([`${entry.owner}/${id}/manifest.json`]);
  if (error) throw new Error('Recording manifest deletion failed');
  await control().store.transact(async (tx) => {
    if (await tx.get('recording', id)) await tx.delete('recording', id);
  });
}

/** Deletes an archived recording's frames and manifest in batches of 100, then its record. Resumable after a crash. */
export async function removeArchive(id, owner) {
  const entry = await ownedRecord(id, owner);
  if (!entry) return;
  assertShared();
  const paths = entry.deletePaths || (await planDeletion(id, owner));
  if (!paths) return;
  await removeAll(paths);
  await removeManifest(entry, id);
}
