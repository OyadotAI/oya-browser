/** POST /control/sessions/:id/control: moving control of a session between the agent and a person. */
import { setTimeout as sleep } from 'node:timers/promises';
import { control, hash, fault } from '../service.ts';
import { registry } from '../../browsers/registry.ts';
import { ACQUIRE_WAIT_MS, TRANSFER_RETRY_MS } from './constants.ts';

/** Marks an attempt that should be retried. */
const RETRY = Symbol('retry');

/** Applies the requested control action, retrying while commands are in flight; returns the new control state. */
export async function transferControl(key, req) {
  const holder = hash(req.authToken);
  // Acquisition waits up to ten seconds for in-flight commands to settle rather than failing at once.
  const deadline = Date.now() + (req.body?.action === 'acquire' ? ACQUIRE_WAIT_MS : 0);
  for (;;) {
    const state = await attempt(key, req, holder).catch((e) => retryable(e, deadline));
    if (state !== RETRY) return state;
    await sleep(TRANSFER_RETRY_MS);
  }
}

/** One try; refuses while the browser still has commands in flight. */
async function attempt(key, req, holder) {
  if (registry.get(req.params.id)?.pending)
    throw fault('commands_pending', 'Wait for in-flight commands to settle before transferring control');
  return control().takeover(key, req.params.id, req.body?.action, holder, { force: req.body?.force === true });
}

/** RETRY for pending commands before the deadline; anything else is rethrown. */
function retryable(e, deadline) {
  if (e.code !== 'commands_pending' || Date.now() >= deadline) throw e;
  return RETRY;
}
