/** Desktop takeover: a person in the console taking control of a browser from the agent and handing it back. */
import { control, hash } from './service.ts';
import { registry } from '../browsers/registry.ts';
import { setTimeout as sleep } from 'node:timers/promises';
import { ACQUIRE_POLL_MS, ACQUIRE_WAIT_MS } from './http/constants.ts';

/** The control-holder identity the desktop uses for a browser. */
export const desktopHolder = (id) => hash(`desktop:${id}`);
/** The control state as the desktop sees it: mode, whether it holds control, and expiry. */
export function desktopState(id, state) {
  return {
    mode: state.mode,
    mine: state.holder === desktopHolder(id),
    expiresAt: state.expiresAt || null,
    taking: !!state.takeover && state.expiresAt > Date.now(),
    revision: state.revision || 0,
  };
}
/** The actions the desktop may take. */
const ACTIONS = ['get', 'request', 'acquire', 'renew', 'return'];
/** Marks an acquire attempt that found commands still running. */
const BUSY = Symbol('busy');
/** Why taking control gave up. */
const STILL_RUNNING =
  'Current action is still running. Automation remains paused; retry taking control or return to agent.';

/**
 * Get, request, acquire, renew or return desktop control of a browser. Acquire waits up to 10 s for the
 * agent's in-flight command to finish; automation stays paused if it does not.
 */
export async function desktopControl(key, id, action, connected = () => true) {
  const holder = desktopHolder(id);
  if (!ACTIONS.includes(action)) throw new Error('Invalid control action');
  if (action === 'get') return desktopState(id, (await control().findSession(key, id)).control);
  if (action === 'return') return giveBack(key, id, holder);
  if (action !== 'acquire') return desktopState(id, await control().takeover(key, id, action, holder));
  return acquire(key, id, holder, connected);
}

/** Returns control to the agent, or resumes automation that was left paused with no holder. */
async function giveBack(key, id, holder) {
  const state = (await control().findSession(key, id)).control;
  const action = state.mode === 'paused' && !state.holder ? 'resume' : 'return';
  return desktopState(id, await control().takeover(key, id, action, holder));
}

/** Requests control, then retries acquiring it until the agent's command finishes or the wait runs out. */
async function acquire(key, id, holder, connected) {
  await control().takeover(key, id, 'request', holder);
  const deadline = Date.now() + ACQUIRE_WAIT_MS;
  do {
    const state = await tryAcquire(key, id, holder, connected);
    if (state !== BUSY) return desktopState(id, state);
    await sleep(ACQUIRE_POLL_MS);
  } while (Date.now() < deadline);
  throw new Error(STILL_RUNNING);
}

/** One acquire; BUSY while a command is still running. */
async function tryAcquire(key, id, holder, connected) {
  if (!connected()) throw new Error('Browser disconnected during takeover');
  if (registry.get(id)?.pending) return BUSY;
  try {
    return await control().takeover(key, id, 'acquire', holder);
  } catch (error) {
    if (error.code !== 'commands_pending') throw error;
    return BUSY;
  }
}
