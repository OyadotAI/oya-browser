/**
 * Unit tests for metering: cost accrued into sessions and their project at
 * most every 30 seconds, budget threshold alerts raised once, and sessions
 * over their own or their project's budget sent to cleanup.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { projectId } from '../../../../../src/modules/control/service.ts';
import { meterRated } from '../../../../../src/modules/control/worker/metering.ts';
import { patchRow, putRow, scratchService } from '../../../support/control.ts';

const A = 'key-a',
  NOW = 1_700_000_000_000,
  HOUR = 3_600_000;
let service;
beforeEach(async () => {
  mock.timers.enable({ apis: ['Date'], now: NOW });
  service = scratchService();
  await service.project(A);
});
afterEach(() => mock.timers.reset());

/** Stores a rated session of the project, created at NOW. */
const session = (id, extra = {}) =>
  putRow(service, 'session', id, {
    id,
    project: projectId(A),
    state: 'ready',
    rateUsdHour: 6,
    costUsd: 0,
    createdAt: NOW,
    ...extra,
  });

/** Meters the listed sessions as the tick would. */
const meter = async (...ids) =>
  meterRated(service, await Promise.all(ids.map((id) => service.store.get('session', id))));
const get = (kind, id) => service.store.get(kind, id);

describe('meterRated', () => {
  it('accrues cost into the session and its project', async () => {
    await session('s');
    mock.timers.tick(HOUR / 2);
    await meter('s');
    assert.equal((await get('session', 's')).costUsd, 3);
    assert.equal((await get('session', 's')).meteredAt, NOW + HOUR / 2);
    assert.equal((await get('project', projectId(A))).costUsd, 3);
  });

  it('waits at least 30 seconds between accruals', async () => {
    await session('s');
    mock.timers.tick(29_000);
    await meter('s');
    assert.equal((await get('session', 's')).costUsd, 0);
  });

  it('skips sessions without a rate or not holding a slot', async () => {
    await session('free', { rateUsdHour: null });
    await session('queued', { state: 'queued' });
    mock.timers.tick(HOUR);
    await meter('free', 'queued');
    assert.equal((await get('project', projectId(A))).costUsd, 0);
  });

  it('sends a session over its own budget to cleanup, saying why', async () => {
    await session('s', { budgetUsd: 1 });
    mock.timers.tick(HOUR);
    await meter('s');
    assert.equal((await get('session', 's')).state, 'cleanup_pending');
    const last = (await service.events(A)).at(-1);
    assert.deepEqual([last.type, last.detail], ['session.cleanup_pending', { reason: 'budget' }]);
  });

  it('raises each budget threshold once', async () => {
    await patchRow(service, 'project', projectId(A), {
      settings: { ...(await get('project', projectId(A))).settings, budgetUsd: 10 },
    });
    await session('s', { rateUsdHour: 9 });
    mock.timers.tick(HOUR);
    await meter('s');
    mock.timers.tick(60_000);
    await meter('s');
    const alerts = (await service.events(A)).filter((e) => e.type === 'budget.threshold');
    assert.deepEqual(
      alerts.map((e) => e.detail.threshold),
      [0.8],
    );
  });

  it('stops managed sessions once the project budget is spent, and leaves unmanaged ones', async () => {
    await patchRow(service, 'project', projectId(A), {
      settings: { ...(await get('project', projectId(A))).settings, budgetUsd: 1 },
    });
    await session('managed', { managed: true });
    await session('attached');
    mock.timers.tick(HOUR);
    await meter('managed', 'attached');
    assert.equal((await get('session', 'managed')).state, 'cleanup_pending');
    assert.equal((await get('session', 'attached')).state, 'ready');
    const thresholds = (await service.events(A)).filter((e) => e.type === 'budget.threshold');
    assert.deepEqual(
      thresholds.map((e) => e.detail.threshold),
      [0.8, 1],
    );
  });

  it('does nothing for a project that no longer exists', async () => {
    await meterRated(service, [{ project: 'prj_gone', rateUsdHour: 1, state: 'ready' }]);
  });
});
