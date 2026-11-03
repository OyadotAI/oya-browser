/** Reads Keel's reporting presence and recent usage separately from Oya Browser metrics. */
import { getConnection } from '../../platform/storage/index.ts';
import { initializeKeelInstalls } from '../telemetry/index.ts';

/** Historical usage is only needed for the selected completed-day window. */
export async function keelRows(since: string) {
  await initializeKeelInstalls();
  const db = getConnection();
  const [installs, usage] = await Promise.all([
    db.select('keel_installs', {}, { order: ['last_seen', 'desc'] }),
    db.select('keel_usage', { day: { gte: since } }),
  ]);
  return { installs, usage };
}
