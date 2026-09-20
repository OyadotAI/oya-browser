/**
 * Private routing settings, kept in the sealed key-settings store. They are
 * deliberately absent from FIELDS, so /config cannot overwrite them.
 */
import { fingerprint as ownerOf } from '../../platform/audit.ts';
import { store, seal, unseal, writeField } from './store.ts';

/** Save a key's routing providers and strategy from the pool. */
export async function saveRouting(apiKey, pool) {
  const owner = ownerOf(apiKey);
  const value = { providers: pool.configs(owner), strategy: pool.strategyFor(owner) };
  await writeField(owner, '_routing', seal(owner, value));
}

/** Re-register one owner's saved providers and strategy. */
function restoreOwner(pool, owner, sealed) {
  try {
    const saved = unseal(owner, sealed);
    for (const cfg of saved.providers || []) pool.register({ ...cfg, owner });
    if (saved.strategy) pool.setStrategy(owner, saved.strategy);
  } catch (err) {
    console.error('[routing] restore failed:', err.message);
  }
}

/** Re-register every key's saved providers and strategy with the pool at startup. */
export function restoreRouting(pool) {
  for (const [owner, fields] of store) {
    if (fields._routing) restoreOwner(pool, owner, fields._routing);
  }
}
