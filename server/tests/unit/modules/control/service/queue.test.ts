/**
 * Unit tests for claimQueued(): failing queued sessions that timed out or are
 * no longer admissible, then promoting the next one that fits, by priority,
 * FIFO within a priority, and rotating between projects.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { instanceId } from '../../../../../src/modules/control/service.ts';
import { scratchService } from '../../../support/control.ts';

const A = 'key-a',
  B = 'key-b',
  NOW = 1_700_000_000_000;
let service;
beforeEach(() => {
  mock.timers.enable({ apis: ['Date'], now: NOW });
  service = scratchService();
});
afterEach(() => mock.timers.reset());

/** Fills the key's single slot with `blocker` and queues `id` behind it. */
async function queueBehind(key, id, request: any = {}, blocker = `${id}-blocker`) {
  if (blocker) await service.reserve(key, { id: blocker, provider: 'cdp', maxConcurrent: 1 });
  mock.timers.tick(1);
  return service.reserve(key, { id, provider: 'cdp', maxConcurrent: 1, request: { queueMs: 60_000, ...request } });
}

/** Ends a session so its slot is free. */
const free = (key, id) => service.complete(key, id, 400, {});

describe('claimQueued', () => {
  it('answers null with nothing queued', async () => {
    assert.equal(await service.claimQueued(), null);
  });

  it('leaves a queued session waiting while its project is still full', async () => {
    await queueBehind(A, 'q');
    assert.equal(await service.claimQueued(), null);
    assert.equal((await service.store.get('session', 'q')).state, 'queued');
  });

  it('promotes a session once a slot frees, with its key and unsealed request', async () => {
    const queued = await queueBehind(A, 'q', { url: 'https://example.com' });
    await free(A, 'q-blocker');
    const job = await service.claimQueued();
    assert.equal(job.key, A);
    assert.equal(job.request.url, 'https://example.com');
    assert.equal(job.request.provider, 'cdp');
    assert.equal(job.session.state, 'provisioning');
    assert.equal(job.session.fence, queued.fence + 1);
    assert.equal(job.session.instance, instanceId);
    assert.equal(job.session.meteredAt, NOW + 1);
    assert.equal((await service.events(A)).at(-1).type, 'session.provisioning');
  });

  it('claims nothing while the fleet drains', async () => {
    await queueBehind(A, 'q');
    await free(A, 'q-blocker');
    await service.drain(true);
    assert.equal(await service.claimQueued(), null);
  });

  it('fails a session whose queue deadline passed', async () => {
    await queueBehind(A, 'q');
    mock.timers.tick(60_000);
    assert.equal(await service.claimQueued(), null);
    const x = await service.store.get('session', 'q');
    assert.deepEqual([x.state, x.errorCode], ['failed', 'queue_timeout']);
  });

  it('fails a session the project’s new budget or policy no longer admits on its runtime', async () => {
    await queueBehind(A, 'q');
    await service.settings(A, { policy: { region: 'eu' } });
    await service.claimQueued();
    assert.equal((await service.store.get('session', 'q')).errorCode, 'unsupported_policy');
  });

  it('runs higher priority first, then oldest first', async () => {
    await queueBehind(A, 'low', { priority: 'low' });
    await queueBehind(A, 'normal-old', {}, null);
    await queueBehind(A, 'high', { priority: 'high' }, null);
    await queueBehind(A, 'normal-new', {}, null);
    await free(A, 'low-blocker');
    const order = [];
    for (let job; (job = await service.claimQueued()); await free(A, job.session.id)) order.push(job.session.id);
    assert.deepEqual(order, ['high', 'normal-old', 'normal-new', 'low']);
  });

  it('takes turns between projects at the same priority', async () => {
    await queueBehind(A, 'a1');
    await queueBehind(A, 'a2', {}, null);
    await queueBehind(B, 'b1');
    await free(A, 'a1-blocker');
    await free(B, 'b1-blocker');
    const first = await service.claimQueued();
    await free(first.key, first.session.id);
    const second = await service.claimQueued();
    assert.notEqual(first.key, second.key);
  });

  it('prices a promoted session at the project’s current rate', async () => {
    await queueBehind(A, 'q');
    await service.settings(A, { rates: { cdp: 6 } });
    await free(A, 'q-blocker');
    const { session } = await service.claimQueued();
    assert.equal(session.rateUsdHour, 6);
    assert.equal(session.reservedCostUsd, 0.1);
  });

  it('holds back a session with its own budget while its provider has no rate', async () => {
    const project = (await service.project(A)).id;
    await service.store.transact(async (tx) => {
      tx.put('session', 'b', {
        id: 'b',
        project,
        state: 'queued',
        provider: 'oya-selfhosted',
        budgetUsd: 1,
        deadline: NOW + 60_000,
        priority: 'normal',
        policies: [],
        fence: 1,
        createdAt: NOW,
      });
    });
    assert.equal(await service.claimQueued(), null);
    assert.equal((await service.store.get('session', 'b')).state, 'queued');
  });
});
