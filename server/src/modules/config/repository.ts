/**
 * Where key settings are kept: the key_settings table, one row per owner and
 * field, in whichever storage driver is configured.
 */
import { getConnection, importLegacyFile, type Connection } from '../../platform/storage/index.ts';
import { dataPath } from '../../platform/paths.ts';

/** The settings file from before storage drivers, imported once on first read. */
export const LEGACY_STORE = dataPath('key-settings.json');
/** The settings table. */
const TABLE = 'key_settings';

/** The keys `owner` still has among `rows`. */
const keysOf = (owner, rows) => rows.filter((r) => r.owner === owner).map((r) => r.key);

/**
 * Replace the given owners' rows: upsert what they hold, then delete the keys
 * they no longer have (a cleared field). Upserting first means a failure part
 * way leaves the old rows, never none.
 */
export async function writeRows(owners, rows, db: Connection = getConnection()) {
  const stamped = rows.map((r) => ({ ...r, updated_at: new Date().toISOString() }));
  if (rows.length) await db.upsert(TABLE, stamped, { update: true });
  await Promise.all(owners.map((owner) => db.delete(TABLE, { owner, key: { notIn: keysOf(owner, rows) } })));
}

/** The legacy file's [owner, fields] entries as rows. */
const legacyRows = (entries) =>
  entries.flatMap(([owner, fields]) => Object.entries(fields || {}).map(([key, value]) => ({ owner, key, value })));

/** Every stored row, after taking in the legacy file if one is still there. */
export async function readRows(db: Connection = getConnection()) {
  await importLegacyFile(LEGACY_STORE, (entries) => db.upsert(TABLE, legacyRows(entries)).then(() => undefined));
  return db.select(TABLE);
}
