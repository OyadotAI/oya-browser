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
const { patchRow, putRow, scratchService } = await import('../../../support/control.ts');
const { AUDIT_RETENTION_FLOOR_DAYS, DAY_MS } = await import('../../../../../src/modules/control/service/constants.ts');

afterEach(() => mock.restoreAll());

/** Appends an event of type 'old' at `at` to each key's project. */
const pushOld = (service, keys, at) =>
  service.store.transact(async (tx) => {
    for (const key of keys) tx.events.push({ project: service.projectIdFor(key), type: 'old', at, detail: {} });
  });

/** Whether the key's project still has an 'old' event. */
const hasOld = async (service, key) => (await service.events(key)).some((e) => e.type === 'old');

describe('maintainControl', () => {
  it('prunes expired rows', async () => {
    const service = scratchService();
    await putRow(service, 'ticket', 't', { expiresAt: Date.now() - 1 });
    await maintainControl(service);
    assert.equal(await service.store.get('ticket', 't'), null);
  });

  it('prunes events older than each project’s own audit window', async () => {
    const service = scratchService();
    // Written first, as an old event was: retention cuts a project's history from the start.
    const pastFloor = Date.now() - (AUDIT_RETENTION_FLOOR_DAYS + 1) * DAY_MS;
    await pushOld(service, ['key-a', 'key-b'], pastFloor);
    await service.settings('key-a', { auditDays: AUDIT_RETENTION_FLOOR_DAYS });
    await service.settings('key-b', { auditDays: AUDIT_RETENTION_FLOOR_DAYS + 2 });
    await maintainControl(service);
    assert.equal(await hasOld(service, 'key-a'), false);
    assert.equal(await hasOld(service, 'key-b'), true);
  });

  it('keeps events for the retention floor even when a stored setting is shorter', async () => {
    const service = scratchService();
    await service.project('key-a');
    await patchRow(service, 'project', service.projectIdFor('key-a'), { settings: { auditDays: 1 } });
    await pushOld(service, ['key-a'], Date.now() - 2 * DAY_MS);
    await maintainControl(service);
    assert.equal(await hasOld(service, 'key-a'), true);
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
