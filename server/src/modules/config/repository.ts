/**
 * Where key settings are kept: the key_settings table, or data/key-settings.json
 * when there is no database or it fails. Callers decide when to fall back.
 */
import { readFile, writeFile, mkdir } from 'fs/promises';
import { dirname } from 'path';
import { db } from '../../platform/db.ts';
import { dataPath } from '../../platform/paths.ts';
import { FILE_INDENT, FILE_MODE } from './constants.ts';

/** The fallback settings file. */
export const STORE = dataPath('key-settings.json');
/** The settings table, one row per owner and field. */
const TABLE = 'key_settings';

/** Throws a database error as a plain Error. */
function check({ error }) {
  if (error) throw new Error(error.message);
}

/** A key quoted for a PostgREST `in` list. */
const quoted = (key) => `"${String(key).replace(/["\\]/g, (c) => '\\' + c)}"`;

/**
 * Replace the given owners' rows: upsert what they hold, then delete the keys
 * they no longer have (a cleared field). Upserting first means a failure part
 * way leaves the old rows, never none.
 */
export async function writeDb(owners, rows, client = db) {
  if (rows.length) {
    const stamped = rows.map((r) => ({ ...r, updated_at: new Date().toISOString() }));
    check(await client.from(TABLE).upsert(stamped, { onConflict: 'owner,key' }));
  }
  await Promise.all(owners.map(async (owner) => check(await staleRows(client, owner, rows))));
}

/** Deletes one owner's rows whose keys are not among `rows`. */
function staleRows(client, owner, rows) {
  const keys = rows.filter((r) => r.owner === owner).map((r) => quoted(r.key));
  const query = client.from(TABLE).delete().eq('owner', owner);
  return keys.length ? query.not('key', 'in', `(${keys.join(',')})`) : query;
}

/** Every stored row from the database. */
export async function readDb() {
  const { data, error } = await db.from(TABLE).select('owner, key, value');
  check({ error });
  return data || [];
}

/** Write every owner's fields to the fallback file. */
export async function writeStoreFile(entries) {
  await mkdir(dirname(STORE), { recursive: true });
  await writeFile(STORE, JSON.stringify(entries, null, FILE_INDENT), { mode: FILE_MODE });
}

/** Every owner's fields from the fallback file. */
export async function readStoreFile() {
  return JSON.parse(await readFile(STORE, 'utf8'));
}
