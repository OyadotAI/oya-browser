/**
 * The private storage bucket recordings are archived to, and the object paths
 * inside it: `<owner>/<sessionId>/<frame>.jpg` and `manifest.json`.
 */
import { dbAuth } from '../../../platform/db.ts';
import { FRAME_DIGITS } from './constants.ts';

/** The private storage bucket recordings are archived to (OYA_RECORDING_BUCKET). */
export const bucket = () => process.env.OYA_RECORDING_BUCKET;
/** Whether recordings are archived to shared storage: needs Supabase and a bucket. */
export const sharedRecordings = () => !!(dbAuth && bucket());
/** The bucket's storage client. */
export const objects = () => dbAuth.storage.from(bucket());
/** Recording ids are UUIDs; anything else never reaches a storage path. */
export const safe = (value) => /^[0-9a-f-]{36}$/i.test(value);
/** File name of frame `index`. */
export const frameName = (index) => `${String(index).padStart(FRAME_DIGITS, '0')}.jpg`;
/** Whether a file in a recording directory belongs in the archive. */
export const archived = (name) => /^\d{6}\.jpg$/.test(name) || name === 'manifest.json';

/** Throw unless the bucket exists and is private. */
export async function assertPrivateBucket() {
  const { data: info, error: bucketError } = await dbAuth.storage.getBucket(bucket());
  if (bucketError || info.public) throw new Error('Recording archive requires an existing private bucket');
}

/** Throw when archive storage is not configured. */
export function assertShared() {
  if (!sharedRecordings()) throw new Error('Recording archive storage unavailable');
}
