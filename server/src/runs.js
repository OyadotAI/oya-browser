/**
 * Background runs for the SDK's submit(): the SDK polls a run and turns its status
 * changes into onSuccess / onFailure / onHumanAttention callbacks. A run that needs
 * a person parks on a promise until someone responds.
 *
 * ponytail: in memory on the replica that took the submission, kept an hour after it
 * ends; pin polls to that replica, or move runs into the control store, before scaling out.
 */

import { randomUUID } from 'crypto';
import { control } from './control/service.js';
import { fingerprint } from './audit.js';

const HUMAN_WAIT_MS = 30 * 60_000;
const KEEP_MS = 60 * 60_000;
const runs = new Map();

const view = ({ owner, waiter, ...run }) => run;

export function start(apiKey, browserId, work) {
  const owner = fingerprint(apiKey);
  const run = { id: `run_${randomUUID()}`, owner, browserId, status: 'running', createdAt: Date.now(), attention: null };
  runs.set(run.id, run);

  // Announce on the project's event log, which is what carries a run to Slack and
  // to customer webhooks. Never on the critical path: a sink that is down or
  // misconfigured must not fail the run it is reporting on.
  const announce = (type, detail) => control()
    .emit(apiKey, type, browserId, { runId: run.id, owner, ...detail })
    .catch((err) => console.error(`[runs] ${type} not recorded:`, err.message));

  const requestHuman = (attention) => new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      Object.assign(run, { waiter: null, attention: null, status: 'running' });
      reject(new Error('Nobody responded within 30 minutes'));
    }, HUMAN_WAIT_MS);
    timer.unref?.();
    Object.assign(run, {
      status: 'needs_attention',
      attention: { id: randomUUID(), at: Date.now(), ...attention },
      waiter: (response) => { clearTimeout(timer); resolve(response); },
    });
    announce('run.needs_attention', { reason: attention?.reason, message: attention?.message });
  });

  work({ requestHuman })
    .then((result) => Object.assign(run, { status: 'succeeded', result }))
    .catch((err) => {
      Object.assign(run, { status: 'failed', error: err.message, errorStatus: err.status || 500 });
      announce('run.failed', { error: err.message });
    })
    .finally(() => {
      Object.assign(run, { endedAt: Date.now(), attention: null, waiter: null });
      setTimeout(() => runs.delete(run.id), KEEP_MS).unref?.();
    });
  return view(run);
}

export function get(owner, id) {
  const run = runs.get(id);
  return run && run.owner === owner ? view(run) : null;
}

/** Answer the run's open attention request. False when it is not waiting for anyone. */
export function respond(owner, id, response = 'done') {
  const run = runs.get(id);
  if (!run || run.owner !== owner || !run.waiter) return false;
  const { waiter } = run;
  Object.assign(run, { waiter: null, attention: null, status: 'running' });
  waiter(String(response));
  return true;
}
