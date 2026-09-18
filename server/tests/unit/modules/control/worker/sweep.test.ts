/**
 * Unit tests for the tick's sweep transaction: sessions whose lease lapsed move
 * on, cleanup is claimed under a lease and a new fence, and gateway leases held
 * by this replica are renewed.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { instanceId } from '../../../../../src/modules/control/service.ts';
import { sweep } from '../../../../../src/modules/control/worker/sweep.ts';
import { sessions as gateways } from '../../../../../src/modules/gateway/service.ts';
import { connectBrowser, disconnectBrowser } from '../../../support/fakes.ts';
import { putRow, scratchService } from '../../../support/control.ts';

const NOW = 1_700_000_000_000;
let service;
beforeEach(() => {
  service = scratchService();
});
afterEach(() => gateways.clear());

/** Stores a session owned by this replica, lease lapsed unless overridden. */
const put = (id, body) =>
  putRow(service, 'session', id, {
    id,
    project: 'p',
    provider: 'oya-cloud',
    instance: instanceId,
    leaseUntil: NOW - 1,
    fence: 1,
    ...body,
  });

/** Runs one sweep at NOW. */
const run = () => service.store.transact((tx) => sweep(tx, NOW));
/** A stored session's state. */
const state = async (id) => (await service.store.get('session', id)).state;

describe('sweep: lapsed leases', () => {
  it('marks a lost dial-in browser disconnected, a lost CDP or gateway attachment stopped', async () => {
    await put('dial', { state: 'ready', provider: 'oya-desktop' });
    await put('cdp', { state: 'ready', provider: 'cdp' });
    await put('gw', { state: 'ready', provider: 'gateway' });
    await run();
    assert.deepEqual(
      [await state('dial'), await state('cdp'), await state('gw')],
      ['disconnected', 'stopped', 'stopped'],
    );
    assert.ok((await service.store.events({ project: 'p' })).some((e) => e.type === 'session.disconnected'));
  });

  it('sends a lost browser with a deletion descriptor to cleanup', async () => {
    await put('s', { state: 'ready', cleanup: { kind: 'sandbox' } });
    await run();
    assert.equal(await state('s'), 'cleanup_pending');
  });

  it('keeps a ready session whose browser is still attached here', async () => {
    await put('s', { state: 'ready' });
    connectBrowser('s');
    try {
      await run();
    } finally {
      disconnectBrowser('s');
    }
    assert.equal(await state('s'), 'ready');
  });

  it('keeps a ready session whose lease has not lapsed', async () => {
    await put('s', { state: 'ready', leaseUntil: NOW + 1 });
    await run();
    assert.equal(await state('s'), 'ready');
  });

  it('moves a lapsed creation to cleanup, failed or unknown by what can exist', async () => {
    await put('described', { state: 'provisioning', cleanup: { kind: 'sandbox' } });
    await put('attach', { state: 'provisioning', provider: 'cdp' });
    await put('vendor', { state: 'provisioning' });
    await run();
    assert.deepEqual(
      [await state('described'), await state('attach'), await state('vendor')],
      ['cleanup_pending', 'failed', 'unknown_outcome'],
    );
  });

  it('summarises every live session for the rest of the tick', async () => {
    await put('s', { state: 'queued', rateUsdHour: 2, updatedAt: 5 });
    const { sessions } = await run();
    assert.deepEqual(sessions, [{ id: 's', project: 'p', state: 'queued', updatedAt: 5, rateUsdHour: 2 }]);
  });
});

describe('sweep: cleanup claims', () => {
  it('claims due cleanup under a two-minute lease and a new fence', async () => {
    await put('s', { state: 'cleanup_pending' });
    const { jobs } = await run();
    assert.equal(jobs.length, 1);
    assert.equal(jobs[0].fence, 2);
    const stored = await service.store.get('session', 's');
    assert.equal(stored.cleanupLease, NOW + 120_000);
    assert.equal(stored.fence, 2);
  });

  it('claims at most eight per tick', async () => {
    for (let i = 0; i < 10; i++) await put(`s${i}`, { state: 'cleanup_pending' });
    assert.equal((await run()).jobs.length, 8);
  });

  it('skips cleanup still backing off, already leased, or with a creation in flight', async () => {
    await put('later', { state: 'cleanup_pending', nextCleanupAt: NOW + 1 });
    await put('leased', { state: 'cleanup_pending', cleanupLease: NOW + 1 });
    await put('creating', { state: 'cleanup_pending', provisioningActive: true, leaseUntil: NOW + 1 });
    assert.deepEqual((await run()).jobs, []);
  });

  it('leaves another live replica to clean up its own session, and takes over once its lease lapses', async () => {
    await put('owned', { state: 'cleanup_pending', instance: 'other', leaseUntil: NOW + 1 });
    await put('orphan', { state: 'cleanup_pending', instance: 'other', leaseUntil: NOW - 1 });
    assert.deepEqual(
      (await run()).jobs.map((j) => j.id),
      ['orphan'],
    );
  });
});

describe('sweep: gateway leases', () => {
  it('renews this replica’s gateway attachments and holds when they run low', async () => {
    await putRow(service, 'attachment', 'g1', { id: 'g1', instance: instanceId, leaseUntil: NOW + 1000 });
    await putRow(service, 'attachment', 'g2', { id: 'g2', instance: instanceId, leaseUntil: NOW + 1000 });
    await putRow(service, 'hold', 'h1', { id: 'h1', resource: 'r', sessionId: 'g1', expiresAt: NOW + 1000 });
    gateways.set('g1', {});
    await run();
    assert.equal((await service.store.get('attachment', 'g1')).leaseUntil, NOW + 120_000);
    assert.equal((await service.store.get('attachment', 'g2')).leaseUntil, NOW + 1000, 'not attached here');
    assert.equal((await service.store.get('hold', 'h1')).expiresAt, NOW + 180_000);
  });
});
