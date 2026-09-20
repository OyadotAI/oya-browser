/**
 * A session's life after admission: provisioning results, patches from its
 * worker, a browser connecting to this replica, and stopping.
 */
import { Status } from '../../../platform/http-status.ts';
import { attachOnly, capacityReached, fault, instanceId, live, moveTo, projectId, stamp, terminal } from './model.ts';
import { ownSession, publicSession, stopSession } from './sessions.ts';
import { atCapacity } from './capacity.ts';
import { CONNECTED_LEASE_MS } from './constants.ts';

/** States a provisioning response can no longer move a session out of. */
const STOPPING_OR_STOPPED = ['cleanup_pending', 'stopping', 'stopped'];
/** States a stopping session may still move to. */
const STOPPING = ['stopping', 'cleanup_pending'];

/** Stop a session; a no-op once it is terminal. */
export async function cancel(store, key, id, { force = false } = {}) {
  return store.transact(async (tx) => {
    const x = await ownSession(tx, key, id);
    if (terminal.has(x.state)) return publicSession(x);
    stopSession(tx, x, { force });
    return publicSession(x);
  });
}

/** Where a failed provisioning call leaves a session: cleanup when a resource may exist, unknown when a provider errored. */
function failedState(x, status) {
  if (x.cleanup) return 'cleanup_pending';
  return status >= Status.INTERNAL && !attachOnly.has(x.provider) ? 'unknown_outcome' : 'failed';
}

/** Where a successful provisioning call leaves a session: ready, unless the browser is still starting. */
const startedState = (x, body) => (x.state === 'ready' || body.status !== 'starting' ? 'ready' : 'provisioning');

/** The state a provisioning response implies. */
const stateAfter = (x, status, body) => (status < Status.BAD_REQUEST ? startedState(x, body) : failedState(x, status));

/** Record the provisioning response and move the session to the state it implies. */
export async function complete(store, key, id, status, body) {
  return store.transact(async (tx) => {
    const x = await ownSession(tx, key, id);
    x.response = { status, body };
    x.provisioningActive = false;
    if (!STOPPING_OR_STOPPED.includes(x.state)) moveTo(tx, x, stateAfter(x, status, body));
    return publicSession(x);
  });
}

/** Throw unless this replica still holds a live provisioning lease on the session. */
export async function assertProvisioning(store, key, id) {
  const x = await store.get('session', id);
  if (!x || x.project !== projectId(key) || !['provisioning', 'ready'].includes(x.state))
    throw fault('creation_cancelled', 'Session creation was cancelled');
  if (x.instance !== instanceId || x.leaseUntil < stamp())
    throw fault('stale_worker', 'Session provisioning lease expired');
}

/** Refuse a stale worker, and a state change out of a terminal or stopping session. */
function assertUpdatable(x, changes, fence) {
  if (fence !== undefined && x.fence !== fence) throw fault('stale_worker', 'Session ownership changed');
  if (terminal.has(x.state) && changes.state && changes.state !== x.state)
    throw fault('terminal_session', 'Session is already terminal');
  if (STOPPING.includes(x.state) && changes.state && ![...STOPPING, 'stopped'].includes(changes.state))
    throw fault('session_stopping', 'Session is stopping');
}

/** Patch a session. `fence` rejects a stale worker; terminal and stopping sessions cannot be moved back. */
export async function update(store, key, id, changes, { fence }: any = {}) {
  return store.transact(async (tx) => {
    const x = await ownSession(tx, key, id);
    assertUpdatable(x, changes, fence);
    const before = x.state;
    Object.assign(x, changes, { updatedAt: stamp() });
    if (before !== x.state) tx.emit(x.project, `session.${x.state}`, id);
    return publicSession(x);
  });
}

/** Refuse a connection to a stopped session, or to one another replica holds. */
function assertConnectable(x) {
  if (terminal.has(x.state) || STOPPING.includes(x.state))
    throw fault('session_stopped', 'Session no longer accepts connections');
  // A provisioning browser enrolls wherever it lands; managed ones are bound by their enrollment token.
  if (x.instance !== instanceId && x.leaseUntil > stamp() && x.state !== 'provisioning')
    throw fault('session_owned', 'Session is connected to another replica');
}

/** A disconnected session gave its slot up: take one again, or refuse at capacity. */
async function retakeSlot(tx, x, o) {
  const p = await tx.get('project', x.project);
  await tx.lock('project', p.id);
  if (atCapacity(await tx.list('session', { project: p.id, states: live }), p, o, o.id)) throw capacityReached();
  x.meteredAt = stamp();
}

/** Mark a session ready on this replica under a new fence. */
function markReady(x, o) {
  Object.assign(x, {
    state: 'ready',
    instance: instanceId,
    leaseUntil: stamp() + CONNECTED_LEASE_MS,
    fence: x.fence + 1,
    persona: o.persona,
    provider: x.provider === 'legacy' ? o.provider : x.provider,
  });
}

/** The connect transaction. */
async function connect(tx, key, o) {
  const x = await ownSession(tx, key, o.id);
  assertConnectable(x);
  if (x.state === 'disconnected') await retakeSlot(tx, x, o);
  markReady(x, o);
  tx.emit(x.project, 'session.ready', o.id);
  return publicSession(x);
}

/** Reserve a connecting browser's session; another path reserving it first is fine. */
async function reserveRacing(service, key, o) {
  try {
    await service.reserve(key, o);
  } catch (e) {
    if (e.code !== 'session_exists') throw e;
  }
}

/** A browser connected to this replica: reserve it if nothing did, re-check capacity after a disconnect, and mark it ready here. */
export async function adopt(service, key, { id, provider, persona = null, maxConcurrent, personaLimit }: any = {}) {
  const o = { id, provider, persona, maxConcurrent, personaLimit };
  // Reserve first if this is a browser connecting without a provisioning operation.
  if (!(await service.store.get('session', id))) await reserveRacing(service, key, o);
  return service.store.transact((tx) => connect(tx, key, o));
}
