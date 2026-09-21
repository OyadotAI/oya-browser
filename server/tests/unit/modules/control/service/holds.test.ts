/**
 * Unit tests for the fleet-wide gates: provider slot holds shared across
 * replicas, and the draining flag.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { scratchService } from '../../../support/control.ts';

const NOW = 1_700_000_000_000;
let service;
beforeEach(() => {
  mock.timers.enable({ apis: ['Date'], now: NOW });
  service = scratchService();
});
afterEach(() => mock.timers.reset());

describe('holdProvider', () => {
  it('holds a slot for three minutes and returns its id', async () => {
    const id = await service.holdProvider('owner', 'vendor', 1);
    assert.deepEqual(await service.store.get('hold', id), {
      id,
      resource: 'owner:vendor',
      expiresAt: NOW + 180_000,
    });
  });

  it('answers 429 provider_capacity when every slot is held', async () => {
    await service.holdProvider('owner', 'vendor', 2);
    await service.holdProvider('owner', 'vendor', 2);
    await assert.rejects(service.holdProvider('owner', 'vendor', 2), { status: 429, code: 'provider_capacity' });
  });

  it('stops counting a hold once it expires', async () => {
    await service.holdProvider('owner', 'vendor', 1);
    mock.timers.tick(180_001);
    await service.holdProvider('owner', 'vendor', 1);
  });

  it('counts holds per owner, and ownerless ones in a shared pool', async () => {
    await service.holdProvider('a', 'vendor', 1);
    await service.holdProvider('b', 'vendor', 1);
    const shared = await service.holdProvider(null, 'vendor', 1);
    assert.equal((await service.store.get('hold', shared)).resource, '@shared:vendor');
  });

  it('frees the slot when released, and ignores an unknown hold', async () => {
    const id = await service.holdProvider('owner', 'vendor', 1);
    await service.releaseProvider(id);
    await service.releaseProvider('unknown');
    await service.holdProvider('owner', 'vendor', 1);
  });
});

describe('drain', () => {
  it('sets and clears the draining flag', async () => {
    assert.equal(await service.drain(1), true);
    assert.equal((await service.store.get('meta', 'draining')).value, true);
    assert.equal(await service.drain(false), false);
    assert.equal((await service.store.get('meta', 'draining')).value, false);
  });
});
