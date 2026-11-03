/** Initializes and refreshes Keel presence without turning duplicate daily reports into new usage. */
import { getConnection, type Connection } from '../../platform/storage/index.ts';
import type { KeelReport } from './keel-batch.ts';

/** Latest metadata is replaced only by an equal or newer server receipt, including a null license. */
export async function recordKeelPresence(identity: KeelReport['identity'], now: string, db = getConnection()) {
  const { install_id, ...metadata } = identity;
  await db.upsert('keel_installs', [{ ...identity, first_seen: now, last_seen: now }]);
  await db.update('keel_installs', { install_id, last_seen: { lte: now } }, { ...metadata, last_seen: now });
  await db.update('keel_installs', { install_id, first_seen: { gte: now } }, { first_seen: now });
}

/** Uses the original receipt and latest nullable metadata when an older local database is opened. */
async function backfillInstall(rows, db: Connection) {
  rows.sort((a, b) => String(a.received_at).localeCompare(String(b.received_at)) || a.day.localeCompare(b.day));
  const first = rows[0],
    last = rows.at(-1);
  const { install_id, version, os, license_id } = last;
  await db.upsert('keel_installs', [
    { install_id, version, os, license_id, first_seen: first.received_at, last_seen: last.received_at },
  ]);
}

/** Postgres backfills in its additive migration; local drivers perform the equivalent idempotent insert. */
export async function initializeKeelInstalls(db = getConnection()) {
  if (db.kind === 'postgres') return;
  const grouped = new Map<string, Record<string, any>[]>();
  for (const row of await db.select('keel_usage', {}))
    grouped.set(row.install_id, [...(grouped.get(row.install_id) ?? []), row]);
  for (const rows of grouped.values()) await backfillInstall(rows, db);
}
