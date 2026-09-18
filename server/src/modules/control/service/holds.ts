/** Fleet-wide gates: provider slot holds shared across replicas, and the draining flag. */
import { randomUUID } from 'node:crypto';
import { Status } from '../../../platform/http-status.ts';
import { fault, stamp } from './model.ts';
import { PROVIDER_HOLD_MS } from './constants.ts';

/** Refuse when every one of the resource's slots is held. */
async function assertFreeSlot(tx, resource, capacity) {
  const held = (await tx.list('hold', { states: [resource] })).filter((h) => h.expiresAt >= stamp());
  if (held.length >= capacity)
    throw fault('provider_capacity', 'Provider capacity is reserved on another replica', Status.TOO_MANY_REQUESTS);
}

/** Hold one of a provider's slots across replicas for three minutes; 429 when all are held. Returns the hold id. */
export async function holdProvider(store, owner, name, capacity) {
  const id = randomUUID(),
    resource = `${owner || '@shared'}:${name}`;
  return store.transact(async (tx) => {
    await tx.lock('meta', `hold:${resource}`);
    await assertFreeSlot(tx, resource, capacity);
    tx.put('hold', id, { id, resource, expiresAt: stamp() + PROVIDER_HOLD_MS });
    return id;
  });
}

/** Give back a provider slot taken with holdProvider. */
export async function releaseProvider(store, id) {
  return store.transact(async (tx) => {
    if (await tx.get('hold', id)) await tx.delete('hold', id);
  });
}

/** Set or clear the fleet-wide draining flag, which stops new admissions. */
export async function drain(store, value) {
  return store.transact(async (tx) => {
    await tx.get('meta', 'draining');
    tx.put('meta', 'draining', { id: 'draining', value: !!value });
    return !!value;
  });
}
