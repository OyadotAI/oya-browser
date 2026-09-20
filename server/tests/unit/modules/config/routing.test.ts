/**
 * Unit tests for a key's saved routing: the pool's providers and strategy are
 * sealed per key and re-registered at startup.
 */
import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { saveRouting, restoreRouting } = await import('../../../../src/modules/config/routing.ts');
const { store, seal } = await import('../../../../src/modules/config/store.ts');
const { fingerprint } = await import('../../../../src/platform/audit.ts');

const KEY = 'config-routing-key';
const OWNER = fingerprint(KEY);

/** A routing pool that records what it is told. */
function fakePool(configs = [], strategy = null) {
  const registered = [];
  const strategies = [];
  return {
    registered,
    strategies,
    configs: () => configs,
    strategyFor: () => strategy,
    register: (cfg) => registered.push(cfg),
    setStrategy: (owner, s) => strategies.push([owner, s]),
  };
}

describe('saved routing', () => {
  beforeEach(() => store.clear());

  it('restores the providers and strategy a key saved, under its owner', async () => {
    await saveRouting(KEY, fakePool([{ id: 'anchor-1', kind: 'anchor' }], 'round-robin'));
    const pool = fakePool();
    restoreRouting(pool);
    assert.deepEqual(pool.registered, [{ id: 'anchor-1', kind: 'anchor', owner: OWNER }]);
    assert.deepEqual(pool.strategies, [[OWNER, 'round-robin']]);
  });

  it('restores no strategy when none was saved', async () => {
    await saveRouting(KEY, fakePool([]));
    const pool = fakePool();
    restoreRouting(pool);
    assert.deepEqual(pool.strategies, []);
  });

  it('skips owners with no routing saved', () => {
    store.set(OWNER, { chat_model: 'm' });
    const pool = fakePool();
    restoreRouting(pool);
    assert.deepEqual(pool.registered, []);
  });

  it('logs and skips routing that no longer unseals, restoring the rest', async () => {
    await saveRouting(KEY, fakePool([{ id: 'ok' }]));
    store.set('other-owner', { _routing: seal('someone-else', {}) });
    const error = mock.method(console, 'error', () => {});
    const pool = fakePool();
    restoreRouting(pool);
    assert.equal(pool.registered.length, 1);
    assert.match(error.mock.calls[0].arguments[0], /restore failed/);
    error.mock.restore();
  });
});
