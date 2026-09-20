/**
 * Unit tests for retention: expired rows and events past each project's audit
 * window are pruned, managed-browser credentials of ended sessions are
 * deleted, and a pass runs at most once a minute.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../../support/data-dir.ts';

ownDataDir('oya-control-maintenance-');
const { hash } = await import('../../../../../src/modules/control/service.ts');
const { maintainControl, startMaintenance } = await import('../../../../../src/modules/control/worker/maintenance.ts');
const { flags } = await import('../../../../../src/modules/control/worker/state.ts');
const { putRow, scratchService } = await import('../../../support/control.ts');

afterEach(() => mock.restoreAll());

describe('maintainControl', () => {
  it('prunes expired rows', async () => {
    const service = scratchService();
    await putRow(service, 'ticket', 't', { expiresAt: Date.now() - 1 });
    await maintainControl(service);
    assert.equal(await service.store.get('ticket', 't'), null);
  });

  it('prunes events older than each project’s own audit window', async () => {
    const service = scratchService();
    await service.settings('key-a', { auditDays: 1 });
    await service.project('key-b');
    const twoDaysAgo = Date.now() - 2 * 86_400_000;
    await service.store.transact(async (tx) => {
      tx.events.push({ project: service.projectIdFor('key-a'), type: 'old', at: twoDaysAgo, detail: {} });
      tx.events.push({ project: service.projectIdFor('key-b'), type: 'old', at: twoDaysAgo, detail: {} });
    });
    await maintainControl(service);
    assert.equal(
      (await service.events('key-a')).some((e) => e.type === 'old'),
      false,
    );
    assert.equal(
      (await service.events('key-b')).some((e) => e.type === 'old'),
      true,
      'default 90 days',
    );
  });

  it('deletes managed-browser credentials whose session ended, and keeps running ones', async () => {
    const service = scratchService();
    await service.reserve('key-a', { id: 'live', provider: 'cdp' });
    await service.reserve('key-a', { id: 'done', provider: 'cdp' });
    const live = await service.enrollmentCredential('key-a', 'live');
    const done = await service.enrollmentCredential('key-a', 'done');
    await service.complete('key-a', 'done', 400, {});
    await maintainControl(service);
    assert.ok(await service.store.get('credential', hash(live.token)));
    assert.equal(await service.store.get('credential', hash(done.token)), null);
  });
});

describe('startMaintenance', () => {
  it('runs in the background at most once a minute', async () => {
    const service = scratchService();
    const prune = mock.method(service.store, 'prune');
    flags.lastMaintenance = 0;
    startMaintenance(service);
    const running = flags.maintenance;
    startMaintenance(service);
    await running;
    assert.equal(flags.maintenance, null);
    startMaintenance(service);
    assert.equal(flags.maintenance, null, 'ran within the last minute');
    assert.equal(prune.mock.callCount(), 1);
  });

  it('logs a failed pass instead of throwing', async () => {
    const error = mock.method(console, 'error', () => {});
    flags.lastMaintenance = 0;
    startMaintenance({
      store: {
        list: async () => {
          throw new Error('db down');
        },
      },
    });
    await flags.maintenance;
    assert.match(error.mock.calls[0].arguments.join(' '), /maintenance: db down/);
  });
});
