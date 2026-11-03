/**
 * Keel usage kept apart from Oya Browser installs. One row per install and
 * reported day makes repeated or concurrent deliveries idempotent.
 */
import { getConnection } from '../../platform/storage/index.ts';
import type { KeelPing } from './keel-ping.ts';
import { recordKeelPresence, initializeKeelInstalls } from './keel-presence.ts';
import type { KeelDay, KeelReport } from './keel-batch.ts';

/** Keeps the first report for this install and usage day, including the initial empty day. */
export async function recordKeelPing(ping: KeelPing) {
  const { day, steps, ...identity } = ping;
  await recordKeelReport({ identity, reports: [{ day, steps, runtime_ms: null, project_ids: null }] });
}

/** Each new measurement becomes immutable independently, preserving any earlier legacy count. */
async function storeDay(identity: KeelReport['identity'], report: KeelDay, now: string) {
  const db = getConnection();
  const key = { install_id: identity.install_id, day: report.day };
  await db.upsert('keel_usage', [{ ...identity, ...report, received_at: now }]);
  for (const field of ['runtime_ms', 'project_ids'])
    if (report[field] !== null) await db.update('keel_usage', { ...key, [field]: null }, { [field]: report[field] });
}

/** Acknowledgement follows every durable write; partial failure remains safe to retry. */
export async function recordKeelReport(report: KeelReport, now = new Date().toISOString()) {
  await initializeKeelInstalls();
  for (const day of report.reports) await storeDay(report.identity, day, now);
  await recordKeelPresence(report.identity, now);
}
