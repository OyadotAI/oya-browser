/**
 * The control plane's background work, run on timers in every replica.
 *
 * Each tick expires lapsed sessions, runs provider cleanup with backoff, meters
 * cost against budgets, starts queued browsers, delivers webhooks and Slack
 * alerts, and occasionally prunes old state. Separate timers renew this
 * replica's leases and re-check the credentials of everything attached to it,
 * so a slow cleanup never lets a lease lapse or a revoked key stay connected.
 * The steps live in worker/; this file runs them.
 */
import { setTimeout as sleep } from 'node:timers/promises';
import { clusterOrigin } from './cluster.ts';
import { control } from './service.ts';
import { metrics } from '../../platform/metrics.ts';
import { exclusive, flags, recordError, workerHealth } from './worker/state.ts';
import { sweep } from './worker/sweep.ts';
import { runCleanup } from './worker/cleanup.ts';
import { meterRated } from './worker/metering.ts';
import { startProvisioning } from './worker/provision.ts';
import { deliver } from './worker/deliveries.ts';
import { startMaintenance, maintainControl } from './worker/maintenance.ts';
import { validateAttachments } from './worker/access.ts';
import { renewLeases } from './worker/leases.ts';
import { DRAIN_POLL_MS, MS_PER_SECOND, TICK_INTERVAL_MS } from './worker/constants.ts';

export { workerHealth, deliver, maintainControl };

/** The worker's interval timers, while running. */
const timers = { tick: null, access: null, lease: null };

/** One pass of the background work; overlapping calls return immediately. */
export async function tick(service = control()) {
  await exclusive('running', () =>
    runTick(service).catch((err) => {
      workerHealth.lastError = err.message;
    }),
  );
}

/** The steps of one tick, in order. */
async function runTick(service) {
  const sessions = await sweepAndClean(service);
  await meterRated(service, sessions);
  startProvisioning(service);
  workerHealth.pendingWebhooks = await deliver(service);
  report(sessions);
  startMaintenance(service);
  workerHealth.lastSuccess = Date.now();
  workerHealth.lastError = null;
}

/** Expires lapsed sessions, runs the cleanup claimed along the way, and returns every live session. */
async function sweepAndClean(service) {
  const now = Date.now();
  // Lease renewal and human-control expiry for local sessions belong to renewLeases, which a slow cleanup cannot delay.
  const { jobs, sessions } = await service.store.transact((tx) => sweep(tx, now));
  await runCleanup(service, jobs, now);
  return sessions;
}

/** Publishes the outstanding cleanup, delivery and queue counts. */
function report(sessions) {
  const cleanup = sessions.filter((x) => x.state === 'cleanup_pending');
  workerHealth.pendingCleanup = cleanup.length;
  metrics.controlCleanup.set({}, cleanup.length);
  metrics.controlWebhooks.set({}, workerHealth.pendingWebhooks);
  metrics.controlQueue.set({}, sessions.filter((x) => x.state === 'queued').length);
  metrics.controlCleanupAge.set({}, Math.max(0, ...cleanup.map((x) => (Date.now() - x.updatedAt) / MS_PER_SECOND)));
}

/** An unref'd interval, so the timers never keep the process alive. */
function every(work) {
  const timer = setInterval(work, TICK_INTERVAL_MS);
  timer.unref();
  return timer;
}

/** Starts the tick, credential and lease timers; resolves once this replica is advertised. */
export function startWorkers() {
  clusterOrigin(); // fail fast on an unroutable cluster configuration
  timers.tick ||= every(() => void tick());
  timers.access ||= every(() => void validateAttachments().catch(() => {}));
  timers.lease ||= every(() => void renewLeases().catch(recordError));
  void tick();
  // Resolves once this replica is advertised and routable.
  return renewLeases().catch(recordError);
}

/** Stops the timers and waits for in-flight work to finish, for shutdown. */
export async function stopWorkers() {
  for (const name of Object.keys(timers)) clearInterval(timers[name]);
  for (const name of Object.keys(timers)) timers[name] = null;
  while (flags.running || flags.validating || flags.heartbeating) await sleep(DRAIN_POLL_MS);
  await flags.provisioning;
  await flags.maintenance;
}
