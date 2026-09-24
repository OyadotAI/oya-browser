/**
 * Where API key digests and account profiles are kept: the api_keys and
 * profiles tables, in whichever storage driver is configured. Only a key's
 * digest ever reaches storage, never the key.
 */
import { getConnection } from '../../platform/storage/index.ts';

/** The api_keys table. */
const KEYS = 'api_keys';
/** The profile columns a person sees. */
const PROFILE_COLUMNS = ['id', 'email', 'display_name', 'role', 'created_at'];

/** The api_keys row for a digest, or null. */
export async function findKey(digest: string) {
  return (await getConnection().select(KEYS, { key_hash: digest }))[0] ?? null;
}

/** Every stored digest. */
export async function keyDigests(): Promise<string[]> {
  return (await getConnection().select(KEYS)).map((row) => row.key_hash);
}

/** Stores key rows; a digest already stored is left as it is. Columns a row leaves out keep their defaults. */
export async function insertKeys(rows: Record<string, unknown>[]) {
  await getConnection().upsert(KEYS, rows);
}

/** Gives an agent's unclaimed key to `userId`, only while nobody owns it; true when this call won it. */
export async function claimAgentKey(digest: string, userId: string) {
  return (await getConnection().update(KEYS, { key_hash: digest, user_id: null }, { user_id: userId })) > 0;
}

/** A user's keys, newest first. */
export function keysOf(userId: string) {
  return getConnection().select(KEYS, { user_id: userId }, { order: ['created_at', 'desc'] });
}

/** Deletes a user's key by digest; true when there was one. */
export async function deleteKey(digest: string, userId: string) {
  return (await getConnection().delete(KEYS, { key_hash: digest, user_id: userId })) > 0;
}

/** Records that a key was just used. */
export async function touchKey(digest: string) {
  await getConnection().update(KEYS, { key_hash: digest }, { last_used_at: new Date().toISOString() });
}

/** Every stored key that has both a project and an owner, for the legacy ownership migration. */
export async function ownedProjects() {
  return (await getConnection().select(KEYS)).filter((row) => row.project && row.user_id);
}

/** Only the profile columns a person sees. */
const visible = (row) => (row ? Object.fromEntries(PROFILE_COLUMNS.map((c) => [c, row[c] ?? null])) : null);

/** A user's profile, or null when there is none. */
export async function findProfile(userId: string) {
  return visible((await getConnection().select('profiles', { id: userId }))[0]);
}

/** Renames a user; the updated profile, or null when there is none. */
export async function renameProfile(userId: string, displayName: string) {
  const changed = await getConnection().update('profiles', { id: userId }, { display_name: displayName });
  return changed ? findProfile(userId) : null;
}
