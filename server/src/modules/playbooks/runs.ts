/**
 * Background runs for the SDK's submit(): the SDK polls a run and turns its status
 * changes into onSuccess / onFailure / onHumanAttention callbacks. A run that needs
 * a person parks on a promise until someone responds.
 *
 * ponytail: in memory on the replica that took the submission, kept an hour after it
 * ends; pin polls to that replica, or move runs into the control store, before scaling out.
 */

import { randomUUID } from 'crypto';
import { control } from '../control/service.ts';
import { fingerprint } from '../../platform/audit.ts';
import { Status } from '../../platform/http-status.ts';
import { HUMAN_WAIT_MS, RUN_KEEP_MS } from './constants.ts';

/** Records one event about a run on the project's event log. */
type Announce = (type: string, detail: object) => void;

/** Runs by id. */
const runs = new Map();

/** A run as its owner sees it: without the owner and the waiting callback. */
const view = ({ owner, waiter, ...run }: Record<string, any>) => run;

/** Start `work` in the background and return the run's public view straight away. */
export function start(apiKey, browserId, work) {
  const owner = fingerprint(apiKey);
  const run: Record<string, any> = newRun(owner, browserId);
  runs.set(run.id, run);
  const announce = announcer(apiKey, browserId, run);
  const requestHuman = (attention) => waitForHuman(run, attention, announce);
  track(run, work({ requestHuman }), announce);
  return view(run);
}

/** A run that has just started. */
function newRun(owner, browserId) {
  return { id: `run_${randomUUID()}`, owner, browserId, status: 'running', createdAt: Date.now(), attention: null };
}

/**
 * Announce on the project's event log, which is what carries a run to Slack and
 * to customer webhooks. Never on the critical path: a sink that is down or
 * misconfigured must not fail the run it is reporting on.
 */
function announcer(apiKey, browserId, run): Announce {
  return (type, detail) =>
    control()
      .emit(apiKey, type, browserId, { runId: run.id, owner: run.owner, ...detail })
      .catch((err) => console.error(`[runs] ${type} not recorded:`, err.message));
}

/** Parks the run on a person until they respond, or fails it after HUMAN_WAIT_MS. */
function waitForHuman(run, attention, announce: Announce) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => giveUp(run, reject), HUMAN_WAIT_MS);
    timer.unref?.();
    park(run, attention, timer, resolve);
    announce('run.needs_attention', { reason: attention?.reason, message: attention?.message });
  });
}

/** Nobody answered in time: the run goes back to running and the wait fails. */
function giveUp(run, reject) {
  Object.assign(run, { waiter: null, attention: null, status: 'running' });
  reject(new Error('Nobody responded within 30 minutes'));
}

/** Marks the run as needing a person, with the callback their answer resolves. */
function park(run, attention, timer, resolve) {
  const waiter = (response) => {
    clearTimeout(timer);
    resolve(response);
  };
  const open = { id: randomUUID(), at: Date.now(), ...attention };
  Object.assign(run, { status: 'needs_attention', attention: open, waiter });
}

/** Records the work's outcome on the run, and forgets the run RUN_KEEP_MS after it ends. */
function track(run, work: Promise<any>, announce: Announce) {
  work
    .then((result) => Object.assign(run, { status: 'succeeded', result }))
    .catch((err) => fail(run, err, announce))
    .finally(() => finish(run));
}

/** The work threw: the run failed with its message and status. */
function fail(run, err, announce: Announce) {
  Object.assign(run, { status: 'failed', error: err.message, errorStatus: err.status || Status.INTERNAL });
  announce('run.failed', { error: err.message });
}

/** The run ended either way: nobody is waiting on it, and it is forgotten after RUN_KEEP_MS. */
function finish(run) {
  Object.assign(run, { endedAt: Date.now(), attention: null, waiter: null });
  setTimeout(() => runs.delete(run.id), RUN_KEEP_MS).unref?.();
}

/** The caller's run, or null when it does not exist or belongs to another key. */
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
