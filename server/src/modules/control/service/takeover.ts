/**
 * Human takeover: operators request, acquire, renew, release and return a
 * browser's control, and agent commands are admitted only while the agent has it.
 */
import { Status } from '../../../platform/http-status.ts';
import { fault, instanceId, stamp } from './model.ts';
import { ownSession } from './sessions.ts';
import { HUMAN_CONTROL_MS, TAKEOVER_REQUEST_MS } from './constants.ts';

/** A handler's answer when control is already where the action would put it. */
const UNCHANGED = Symbol('unchanged');

/** Whether a live human hold belongs to someone other than `holder`. */
const heldByOther = (c, holder) => c.mode === 'human' && c.expiresAt > stamp() && c.holder !== holder;
/** Refusal while another operator holds the browser. */
const busy = (message = 'Another operator has control') => fault('control_busy', message);

/** Ask for control: pause the agent briefly while the operator's page connects. */
function request(x, holder) {
  if (heldByOther(x.control, holder)) throw busy();
  if (x.control.mode !== 'human' || x.control.expiresAt <= stamp())
    x.control = { mode: 'paused', holder, takeover: true, expiresAt: stamp() + TAKEOVER_REQUEST_MS };
}

/** Extend the holder's human control. */
function renew(x, holder) {
  if (x.control.mode !== 'human' || x.control.holder !== holder || x.control.expiresAt <= stamp())
    throw busy('Human control has expired or changed');
  x.control.expiresAt = stamp() + HUMAN_CONTROL_MS;
}

/** Hand control back to the agent; only the current operator may. */
function giveBack(x, holder) {
  if (x.control.mode === 'agent') return UNCHANGED;
  if (x.control.holder !== holder || !['human', 'paused'].includes(x.control.mode))
    throw busy('Only the current operator can return control');
  x.control = { mode: 'agent' };
}

/** Take human control once in-flight commands settle; `force` takes it from another operator. */
function acquire(x, holder, force) {
  if (x.inFlight > 0) throw fault('commands_pending', 'In-flight commands must settle before takeover');
  if (!force && heldByOther(x.control, holder)) throw busy();
  x.control = { mode: 'human', holder, expiresAt: stamp() + HUMAN_CONTROL_MS };
}

/** Let go of human control, leaving the agent paused. */
function release(x, holder) {
  if (x.control.mode === 'human' && x.control.holder !== holder) throw busy();
  x.control = { mode: 'paused' };
}

/** Give the agent control again, once no human holds it. */
function resume(x) {
  if (x.control.mode === 'human' && x.control.expiresAt > stamp()) throw busy('Human control must be released first');
  x.control = { mode: 'agent' };
}

/** Each takeover action's handler. */
const ACTIONS = { request, renew, return: giveBack, acquire, release, resume };

/** The handler for `action`; 400 for an unknown one. */
function handlerFor(action) {
  if (!Object.hasOwn(ACTIONS, action))
    throw fault('invalid_action', 'Use acquire, release, or resume', Status.BAD_REQUEST);
  return ACTIONS[action];
}

/** Refuse unless the session is ready and no other operator is mid-takeover. */
function assertTakeable(x, holder) {
  if (x.state !== 'ready') throw fault('not_ready', 'Session must be ready');
  if (x.control.takeover && x.control.expiresAt > stamp() && x.control.holder !== holder)
    throw busy('Another operator is taking control');
}

/** The takeover transaction: apply the action, stamp a new revision and announce the new mode. */
async function applyTakeover(tx, key, id, action, holder, force) {
  const x = await ownSession(tx, key, id);
  const revision = Math.max(stamp(), (x.control.revision || 0) + 1);
  assertTakeable(x, holder);
  if (handlerFor(action)(x, holder, force) === UNCHANGED) return x.control;
  x.control.revision = revision;
  tx.emit(x.project, `control.${x.control.mode}`, id);
  return x.control;
}

/**
 * `force` takes the browser from whoever is holding it. The hold exists so two
 * operators do not fight over one page, not to lock out the project that owns it:
 * a tab that closed without releasing, or a lease renewed by a forgotten dialog,
 * otherwise blocks its owner for five minutes with nothing they can do about it.
 */
export async function takeover(store, key, id, action, holder, { force = false } = {}) {
  return store.transact((tx) => applyTakeover(tx, key, id, action, holder, force));
}

/** Admit a browser command under the current control mode. Returns the function that settles it; calling it twice is harmless. */
export async function beginCommand(store, id, holder = null) {
  const fence = await store.beginCommand(id, holder, instanceId);
  let ended = false;
  return async () => {
    if (ended) return;
    ended = true;
    await store.finishCommand(id, fence);
  };
}
