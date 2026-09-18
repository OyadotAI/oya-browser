/**
 * Unit tests for settleCosts, the commit hook that bills a session's last
 * unmetered interval whenever it stops holding capacity.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { projectId } from '../../../../../src/modules/control/service.ts';
import { readySession, scratchService } from '../../../support/control.ts';

const A = 'key-a',
  NOW = 1_700_000_000_000,
  HOUR = 3_600_000;
let service;
beforeEach(() => {
  mock.timers.enable({ apis: ['Date'], now: NOW });
  service = scratchService();
});
afterEach(() => mock.timers.reset());

describe('settleCosts', () => {
  it('bills a rated session and its project when it stops', async () => {
    await service.settings(A, { rates: { cdp: 6 } });
    await readySession(service, A, 's');
    mock.timers.tick(HOUR / 2);
    await service.cancel(A, 's', { force: true });
    assert.equal((await service.store.get('session', 's')).costUsd, 3);
    assert.equal((await service.store.get('project', projectId(A))).costUsd, 3);
  });

  it('bills only the time since the session was last metered', async () => {
    await service.settings(A, { rates: { cdp: 6 } });
    await readySession(service, A, 's');
    await service.update(A, 's', { meteredAt: NOW + HOUR });
    mock.timers.tick(2 * HOUR);
    await service.cancel(A, 's', { force: true });
    assert.equal((await service.store.get('session', 's')).costUsd, 6);
  });

  it('bills nothing for a session without a rate', async () => {
    await readySession(service, A, 's');
    mock.timers.tick(HOUR);
    await service.cancel(A, 's', { force: true });
    assert.equal((await service.store.get('session', 's')).costUsd, 0);
  });

  it('bills nothing for a change that keeps the session holding its slot', async () => {
    await service.settings(A, { rates: { cdp: 6 } });
    await readySession(service, A, 's');
    mock.timers.tick(HOUR);
    await service.update(A, 's', { note: 1 });
    assert.equal((await service.store.get('session', 's')).costUsd, 0);
  });
});
