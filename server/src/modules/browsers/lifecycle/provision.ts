/**
 * Launching a batch of Oya Cloud sandboxes, a bounded number at a time, and
 * booking the outcome.
 */
import { createSandbox } from '../../../drivers/sandbox.ts';
import { metrics } from '../../../platform/metrics.ts';
import { audit } from '../../../platform/audit.ts';
import * as usage from '../../../platform/usage.ts';
import { PROVISION_IN_FLIGHT } from '../constants.ts';

/** The outcome of one create, shaped like Promise.allSettled's. */
async function settle(work: () => Promise<unknown>) {
  try {
    return { status: 'fulfilled', value: await work() };
  } catch (reason) {
    return { status: 'rejected', reason };
  }
}

/** One worker: takes creates off the queue until it is empty. */
async function drain(queue: number[], results: any[], key: string, name?: string) {
  while (queue.length) {
    queue.shift();
    results.push(await settle(() => createSandbox({ apiKey: key, name })));
  }
}

/**
 * Bounded concurrency: a fleet is built by repeating this call, and firing
 * every create at once would just rate-limit us at the provider.
 */
export async function createMany(key: string, name: string | undefined, count: number) {
  const results = [];
  const queue = Array.from({ length: count }, (_, i) => i);
  await Promise.all(
    Array.from({ length: Math.min(PROVISION_IN_FLIGHT, count) }, () => drain(queue, results, key, name)),
  );
  const created = results.filter((r) => r.status === 'fulfilled').map((r) => r.value);
  const failed = results.filter((r) => r.status === 'rejected').map((r) => r.reason?.message || 'unknown error');
  return { created, failed };
}

/** Logs failures and records usage and metrics for a provision. */
export function bookProvision(key: string, count: number, created: unknown[], failed: string[]) {
  if (failed.length) console.error(`[sandbox] ${failed.length}/${count} failed:`, failed.join('; '));
  usage.record(key, 'sandboxes_created', created.length);
  metrics.sandboxes.inc({ op: 'create', outcome: 'ok' }, created.length);
  if (failed.length) metrics.sandboxes.inc({ op: 'create', outcome: 'error' }, failed.length);
}

/** Audits a provision; it failed when nothing was created. */
export function auditProvision(req, key: string, count: number, created: unknown[], failed: string[]) {
  const meta = { requested: count, created: created.length, failed: failed.length };
  const outcome = created.length ? 'ok' : 'error';
  audit({ action: 'browser.provision', actorKey: key, targetType: 'sandbox', outcome, meta, req });
}
