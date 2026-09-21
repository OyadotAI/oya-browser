/**
 * Provider cleanup for the sessions a tick claimed: release the resource, drop
 * the connection, mark the session stopped, or back off and retry.
 */
import { attachOnly } from '../service.ts';
import { openText } from '../../../platform/secrets.ts';
import { registry } from '../../browsers/registry.ts';
import { sessions as gateways } from '../../gateway/service.ts';
import { releasePersisted } from '../../../drivers/providers.ts';
import { removeManaged } from '../managed.ts';
import { removeSandbox } from '../../../drivers/sandbox.ts';
import { workerHealth } from './state.ts';
import { backoff } from './backoff.ts';
import { CLEANUP_BACKOFF_CAP_MS, CLEANUP_BACKOFF_MAX_EXPONENT, CloseCode } from './constants.ts';

/** How each kind of cleanup descriptor releases its resource. */
const RELEASE = {
  docker: (c, key, x) => removeManaged(c.container, key, x.id, c.daemonId, c.runtime, c.namespace),
  sandbox: (c, key) => removeSandbox(c.browserId, key),
  vendor: (c) => releasePersisted(c),
};

/** Cleans up each claimed job in turn. */
export async function runCleanup(service, jobs, now) {
  const keys = await projectKeys(service, jobs);
  for (const x of jobs) await cleanupJob(service, x, keys, now);
}

/** The sealed API key of every project the jobs belong to. */
async function projectKeys(service, jobs) {
  const refs = [...new Set(jobs.map((x) => x.project))].map((id) => ({ kind: 'project', id }));
  return new Map((await service.store.load(refs)).flat().map((r) => [r.id, r.body.key]));
}

/** Releases one session's resource and marks it stopped, or schedules a retry. */
async function cleanupJob(service, x, keys, now) {
  const opened = openKey(x, keys);
  if (!opened) return;
  try {
    await stopResource(x, opened.key);
    await service.update(opened.key, x.id, { state: 'stopped', cleanupLease: null }, { fence: x.fence });
  } catch {
    await retryLater(service, opened.key, x, now);
  }
}

/** The project's key, or null (noted for health) when it cannot be decrypted. */
function openKey(x, keys) {
  // One project sealed under another secret must not stall every other project's cleanup; its lease lapses and it retries.
  try {
    return { key: openText(`control:${x.project}`, keys.get(x.project)) };
  } catch {
    workerHealth.lastError = `Project ${x.project} key could not be decrypted`;
    return null;
  }
}

/** Releases the provider resource, then drops any gateway session or browser connection this replica holds. */
async function stopResource(x, key) {
  await releaseProvider(x, key);
  if (gateways.has(x.id)) await gateways.get(x.id).destroy('Stopped by control plane');
  if (registry.get(x.id)) closeBrowser(x.id);
}

/** Runs the descriptor's release; with none, only an attach-only or still-connected session is safe to stop. */
async function releaseProvider(x, key) {
  const kind = x.cleanup?.kind;
  if (Object.hasOwn(RELEASE, kind)) return RELEASE[kind](x.cleanup, key, x);
  if (!attachOnly.has(x.provider) && !registry.get(x.id) && !gateways.get(x.id))
    throw new Error('Resource outcome is unknown; operator reconciliation required');
}

/** Closes and forgets the browser's socket. */
function closeBrowser(id) {
  try {
    registry.get(id).ws?.close(CloseCode.STOPPED, 'Stopped by control plane');
  } catch {}
  registry.remove(id);
}

/** Releases the lease and backs off; a failed write is left for the lease to lapse. */
async function retryLater(service, key, x, now) {
  const attempts = x.cleanupAttempts || 0;
  const changes = {
    cleanupLease: null,
    cleanupAttempts: attempts + 1,
    nextCleanupAt: now + backoff(attempts, CLEANUP_BACKOFF_MAX_EXPONENT, CLEANUP_BACKOFF_CAP_MS),
    cleanupError: 'Provider cleanup failed; retry pending',
  };
  await service.update(key, x.id, changes, { fence: x.fence }).catch(() => {});
}
