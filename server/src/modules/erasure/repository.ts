/**
 * Where erasure deletes from: every table keyed by a project's owner
 * fingerprint or id, the rows that belong to a person, and their sign-in.
 * audit_log is not touched: it holds fingerprints, and must outlive what it records.
 */
import { getConnection, type Connection } from '../../platform/storage/index.ts';
import { dbAuth } from '../../platform/db.ts';

/** Tables keyed by the owner fingerprint or the project id, and the filter that picks a project's rows. */
const projectTables = (owner: string, project: string): [string, Record<string, string>][] => [
  ['personas', { owner }],
  ['routines', { owner }],
  ['key_settings', { owner }],
  ['usage', { api_key: owner }],
  ['api_keys', { project }],
];

/** Sealed per-persona records filed as `personaId` or `personaId|site`. */
const PERSONA_RECORDS = ['persona_credentials', 'mfa_factors'];

/** Whether a record id is filed under one of these personas. */
const filedUnder = (id: string, personaIds: string[]) => personaIds.some((p) => id === p || id.startsWith(`${p}|`));

/** Deletes a project's personas with their logins, credentials and factors, then its other rows. */
export async function deleteProjectRows(owner: string, project: string, db: Connection = getConnection()) {
  const personaIds = (await db.select('personas', { owner })).map((r) => String(r.id));
  for (const id of personaIds) await db.delete('persona_logins', { id });
  for (const table of PERSONA_RECORDS) await deleteFiledUnder(db, table, personaIds);
  for (const [table, where] of projectTables(owner, project)) await db.delete(table, where);
}

/** Deletes a record table's rows filed under these personas. */
async function deleteFiledUnder(db: Connection, table: string, personaIds: string[]) {
  if (!personaIds.length) return;
  const ids = (await db.select(table)).map((r) => String(r.id)).filter((id) => filedUnder(id, personaIds));
  for (const id of ids) await db.delete(table, { id });
}

/** Deletes a person's keys and profile. Their subscription row stays: it is the billing record. */
export async function deleteUserRows(userId: string, db: Connection = getConnection()) {
  await db.delete('api_keys', { user_id: userId });
  await db.delete('profiles', { id: userId });
}

/** A person's subscription row, or null. */
export async function subscriptionOf(userId: string, db: Connection = getConnection()) {
  return (await db.select('subscriptions', { user_id: userId }))[0] ?? null;
}

/** Deletes the person's sign-in, where sign-in is Supabase; nothing to do on an API-key-only server. */
export async function deleteSignIn(userId: string) {
  if (!dbAuth) return;
  const { error } = await dbAuth.auth.admin.deleteUser(userId);
  if (error) throw new Error(`Sign-in not deleted: ${error.message}`);
}
