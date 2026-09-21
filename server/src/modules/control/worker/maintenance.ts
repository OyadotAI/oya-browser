/**
 * Retention, run at most once a minute: expired rows, events past each
 * project's audit window, credentials of ended managed browsers, recordings.
 */
import { control, terminal } from '../service.ts';
import { maintain as maintainRecordings } from '../../gateway/recorder.ts';
import { flags } from './state.ts';
import { DAY_MS, DEFAULT_AUDIT_DAYS, MAINTENANCE_INTERVAL_MS } from './constants.ts';

/** Starts a maintenance pass in the background unless one is running or ran within the interval. */
export function startMaintenance(service) {
  if (flags.maintenance || !(Date.now() - flags.lastMaintenance > MAINTENANCE_INTERVAL_MS)) return;
  flags.lastMaintenance = Date.now();
  flags.maintenance = maintainControl(service)
    .catch((e) => console.error('[control] maintenance:', e.message))
    .finally(() => {
      flags.maintenance = null;
    });
}

/** Retention: expired rows, events past each project's audit window, credentials of ended managed browsers, recordings. */
export async function maintainControl(service = control()) {
  const now = Date.now();
  const projects = await service.store.list('project');
  await service.store.prune(now, auditCutoffs(projects, now));
  await forgetEndedCredentials(service);
  await maintainRecordings();
}

/** Each project's oldest event to keep. */
const auditCutoffs = (projects, now) =>
  Object.fromEntries(projects.map((p) => [p.id, now - (p.settings.auditDays || DEFAULT_AUDIT_DAYS) * DAY_MS]));

/** Deletes enrolment credentials whose browser session has ended. */
async function forgetEndedCredentials(service) {
  const enrolled = await service.store.list('credential', { states: ['browser'] });
  if (!enrolled.length) return;
  const running = await runningSessions(service, enrolled);
  const ended = enrolled.filter((c) => !running.has(c.sessionId)).map((c) => c.digest);
  if (ended.length) await service.store.transact((tx) => deleteCredentials(tx, ended));
}

/** Ids of the credentials' sessions that have not ended. */
async function runningSessions(service, enrolled) {
  const rows = await service.store.load(enrolled.map((c) => ({ kind: 'session', id: String(c.sessionId) })));
  return new Set(
    rows
      .flat()
      .filter((r) => !terminal.has(r.body.state))
      .map((r) => r.id),
  );
}

/** Deletes the credentials with these digests. */
async function deleteCredentials(tx, digests) {
  await tx.getMany('credential', digests);
  for (const digest of digests) await tx.delete('credential', digest);
}
