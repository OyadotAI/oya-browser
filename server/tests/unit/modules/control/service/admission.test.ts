/**
 * Unit tests for reserve(), admission before any provider is called:
 * idempotency replay, drain, duplicate ids, capacity and queueing, the hourly
 * cloud quota, governance and budgets, and that one project's sessions never
 * count against another's.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { instanceId, projectId } from '../../../../../src/modules/control/service.ts';
import { openText } from '../../../../../src/platform/secrets.ts';
import { MAX_QUEUED } from '../../../../../src/modules/control/service/constants.ts';
import { putRow, scratchService } from '../../../support/control.ts';

const A = 'key-a',
  B = 'key-b',
  NOW = 1_700_000_000_000,
  HOUR = 3_600_000;
let service;
beforeEach(() => {
  mock.timers.enable({ apis: ['Date'], now: NOW });
  service = scratchService();
});
afterEach(() => mock.timers.reset());

/** reserve() with a cdp provider unless overridden. */
const reserve = (key, options: any = {}) => service.reserve(key, { provider: 'cdp', ...options });

describe('reserve: admitted', () => {
  it('records a provisioning session leased to this replica under agent control', async () => {
    const x = await reserve(A, { id: 's1', persona: 'p-1' });
    assert.equal(x.state, 'provisioning');
    assert.equal(x.project, projectId(A));
    assert.equal(x.instance, instanceId);
    assert.equal(x.fence, 1);
    assert.equal(x.leaseUntil, NOW + 180_000);
    assert.deepEqual(x.control, { mode: 'agent' });
    assert.equal(x.provisioningActive, true);
    assert.equal(x.runtimeRequired, false);
    assert.equal((await service.events(A)).at(-1).type, 'session.provisioning');
  });

  it('makes up an id when the caller gives none', async () => {
    assert.match((await reserve(A)).id, /^[0-9a-f-]{36}$/);
  });
});

describe('reserve: idempotency', () => {
  it('replays the session an earlier identical request created', async () => {
    const first = await reserve(A, { id: 's1', idempotencyKey: 'k', request: { a: 1, b: 2 } });
    const again = await reserve(A, { id: 's2', idempotencyKey: 'k', request: { b: 2, a: 1 } });
    assert.equal(again.id, first.id);
    assert.equal(again.replay, true);
  });

  it('refuses the same key for a different request', async () => {
    await reserve(A, { id: 's1', idempotencyKey: 'k', request: { a: 1 } });
    await assert.rejects(reserve(A, { id: 's2', idempotencyKey: 'k', request: { a: 2 } }), {
      status: 409,
      code: 'idempotency_conflict',
    });
  });

  it('stops replaying after seven days', async () => {
    await reserve(A, { id: 's1', idempotencyKey: 'k' });
    mock.timers.tick(7 * 24 * HOUR + 1);
    assert.equal((await reserve(A, { id: 's2', idempotencyKey: 'k' })).id, 's2');
  });

  it('scopes keys to their project, so another tenant’s key never replays ours', async () => {
    await reserve(A, { id: 's1', idempotencyKey: 'k' });
    const theirs = await reserve(B, { id: 's2', idempotencyKey: 'k' });
    assert.equal(theirs.id, 's2');
    assert.equal(theirs.replay, undefined);
  });

  it('refuses a key over 200 characters', async () => {
    await assert.rejects(reserve(A, { idempotencyKey: 'k'.repeat(201) }), {
      status: 400,
      code: 'invalid_idempotency_key',
    });
  });
});

describe('reserve: refusals before capacity', () => {
  it('refuses while the fleet drains', async () => {
    await service.drain(true);
    await assert.rejects(reserve(A), { status: 503, code: 'draining' });
  });

  it('refuses an id that is already taken, even by another project', async () => {
    await reserve(B, { id: 's1' });
    await assert.rejects(reserve(A, { id: 's1' }), { status: 409, code: 'session_exists' });
  });

  it('refuses a queue wait outside 0–300000 ms', async () => {
    for (const queueMs of [-1, 300_001, 1.5])
      await assert.rejects(reserve(A, { request: { queueMs } }), { status: 400, code: 'invalid_queue' });
  });

  it('refuses a session budget that is not positive', async () => {
    await assert.rejects(reserve(A, { request: { budgetUsd: 0 } }), { status: 400, code: 'invalid_budget' });
  });

  it('refuses a malformed request policy', async () => {
    await assert.rejects(reserve(A, { request: { policy: { nope: 1 } } }), { status: 400, code: 'invalid_policy' });
  });
});

describe('reserve: capacity', () => {
  it('refuses at the caller’s concurrency cap', async () => {
    await reserve(A, { maxConcurrent: 1 });
    await assert.rejects(reserve(A, { maxConcurrent: 1 }), { status: 429, code: 'quota_exceeded' });
  });

  it('applies the tighter of the project’s and the caller’s caps', async () => {
    await service.settings(A, { maxConcurrent: 1 });
    await reserve(A, { maxConcurrent: 10 });
    await assert.rejects(reserve(A, { maxConcurrent: 10 }), { code: 'quota_exceeded' });
  });

  it('refuses past a persona’s own limit', async () => {
    await reserve(A, { persona: 'p', personaLimit: 1 });
    await reserve(A, { persona: 'q', personaLimit: 1 });
    await assert.rejects(reserve(A, { persona: 'p', personaLimit: 1 }), { code: 'quota_exceeded' });
  });

  it('never counts another project’s sessions', async () => {
    await reserve(B, { maxConcurrent: 1 });
    assert.equal((await reserve(A, { maxConcurrent: 1 })).state, 'provisioning');
  });

  it('queues when full and the request may wait, with its deadline, priority and sealed request', async () => {
    await reserve(A, { maxConcurrent: 1 });
    const x = await reserve(A, {
      id: 'q1',
      maxConcurrent: 1,
      persona: 'p',
      request: { queueMs: 5000, priority: 'high', url: 'u' },
    });
    assert.equal(x.state, 'queued');
    assert.equal(x.provisioningActive, false);
    assert.equal(x.deadline, NOW + 5000);
    assert.equal(x.priority, 'high');
    assert.equal(x.reservedCostUsd, 0);
    assert.deepEqual(openText('queue:q1', x.queuedRequest), {
      queueMs: 5000,
      priority: 'high',
      url: 'u',
      provider: 'cdp',
      persona: 'p',
      profile: 'p',
    });
  });

  it('queues with normal priority when the caller names an unknown one', async () => {
    await reserve(A, { maxConcurrent: 1 });
    const x = await reserve(A, { maxConcurrent: 1, request: { queueMs: 1, priority: 'urgent' } });
    assert.equal(x.priority, 'normal');
  });

  it('refuses once the project already has 1000 queued', async () => {
    await reserve(A, { maxConcurrent: 1 });
    await service.store.transact(async (tx) => {
      for (let i = 0; i < MAX_QUEUED; i++)
        tx.put('session', `q${i}`, { id: `q${i}`, project: projectId(A), state: 'queued' });
    });
    await assert.rejects(reserve(A, { maxConcurrent: 1, request: { queueMs: 1000 } }), {
      status: 429,
      code: 'queue_full',
    });
  });
});

describe('reserve: hourly cloud quota', () => {
  it('refuses oya-cloud starts past the hourly limit, and allows them an hour later', async () => {
    await reserve(A, { provider: 'oya-cloud', hourlyLimit: 1 });
    await assert.rejects(reserve(A, { provider: 'oya-cloud', hourlyLimit: 1 }), {
      status: 429,
      code: 'hourly_quota',
    });
    mock.timers.tick(HOUR + 1);
    assert.equal((await reserve(A, { provider: 'oya-cloud', hourlyLimit: 1 })).state, 'provisioning');
  });

  it('does not count other providers against it', async () => {
    await reserve(A, { hourlyLimit: 1 });
    await reserve(A, { provider: 'oya-cloud', hourlyLimit: 1 });
  });
});

describe('reserve: governance and budgets', () => {
  it('refuses governance on anything but the managed self-hosted runtime', async () => {
    await assert.rejects(reserve(A, { request: { policy: { region: 'eu' } } }), {
      status: 422,
      code: 'runtime_not_verified',
    });
    await assert.rejects(reserve(A, { request: { governed: true } }), { code: 'runtime_not_verified' });
    await assert.rejects(reserve(A, { inheritedPolicies: [{ region: 'eu' }] }), { code: 'runtime_not_verified' });
    await service.settings(A, { policy: { region: 'eu' } });
    await assert.rejects(reserve(A), { code: 'runtime_not_verified' });
  });

  it('refuses budgets on anything but the managed self-hosted runtime', async () => {
    await assert.rejects(reserve(A, { request: { budgetUsd: 1 } }), { status: 422, code: 'unsupported_budget' });
  });

  it('refuses a session budget without a managed browser and a rate', async () => {
    const o = { provider: 'oya-selfhosted', request: { budgetUsd: 1 } };
    await assert.rejects(reserve(A, o), { code: 'unsupported_budget' });
    await assert.rejects(reserve(A, { ...o, managed: true }), { code: 'unsupported_budget' });
  });

  it('refuses a project budget for an unmanaged browser, or without a rate card', async () => {
    await service.settings(A, { budgetUsd: 1 });
    await assert.rejects(reserve(A, { provider: 'oya-selfhosted' }), { code: 'unsupported_budget' });
    await assert.rejects(reserve(A, { provider: 'oya-selfhosted', managed: true }), {
      status: 429,
      code: 'budget_admission',
      message: 'A rate card is required for budgeted sessions',
    });
  });

  it('admits a budgeted managed session, reserving a minute of its rate and combining its policies', async () => {
    await service.settings(A, { budgetUsd: 10, rates: { 'oya-selfhosted': 6 }, policy: { region: 'eu' } });
    const x = await reserve(A, {
      provider: 'oya-selfhosted',
      managed: true,
      inheritedPolicies: [{ redactRecording: true }],
      request: { budgetUsd: 2, policy: { allowedHosts: ['a.com'] } },
    });
    assert.equal(x.reservedCostUsd, 0.1);
    assert.equal(x.rateUsdHour, 6);
    assert.equal(x.budgetUsd, 2);
    assert.equal(x.runtimeRequired, true);
    assert.deepEqual(x.policies, [{ redactRecording: true }, { region: 'eu' }, { allowedHosts: ['a.com'] }]);
  });

  it('refuses once spend plus reservations would pass the project budget', async () => {
    await service.settings(A, { budgetUsd: 1, rates: { 'oya-selfhosted': 60 } });
    await reserve(A, { provider: 'oya-selfhosted', managed: true });
    await assert.rejects(reserve(A, { provider: 'oya-selfhosted', managed: true }), {
      code: 'budget_admission',
      message: 'Project budget exhausted',
    });
  });

  it('counts metered spend already on the project', async () => {
    await service.settings(A, { budgetUsd: 1, rates: { 'oya-selfhosted': 6 } });
    await putRow(service, 'project', projectId(A), {
      ...(await service.store.get('project', projectId(A))),
      costUsd: 1,
    });
    await assert.rejects(reserve(A, { provider: 'oya-selfhosted', managed: true }), { code: 'budget_admission' });
  });
});
