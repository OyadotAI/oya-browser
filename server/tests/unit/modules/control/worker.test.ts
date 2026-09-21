/**
 * Unit tests for the control worker: one tick expires lapsed sessions, claims
 * and runs cleanup, and records its health; the timers start and stop cleanly.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';

ownDataDir('oya-control-worker-');
const { instanceId } = await import('../../../../src/modules/control/service.ts');
const { tick, workerHealth, startWorkers, stopWorkers } = await import('../../../../src/modules/control/worker.ts');
const { flags } = await import('../../../../src/modules/control/worker/state.ts');
const { putRow, scratchService } = await import('../../support/control.ts');

afterEach(async () => {
  await flags.maintenance;
  await flags.provisioning;
  mock.restoreAll();
});

describe('tick', () => {
  it('expires a lapsed session, stops its attach-only cleanup and records success', async () => {
    const service = scratchService();
    await service.project('key-a');
    await putRow(service, 'session', 's', {
      id: 's',
      project: service.projectIdFor('key-a'),
      provider: 'cdp',
      state: 'ready',
      cleanup: { kind: 'none' },
      instance: instanceId,
      leaseUntil: 0,
      fence: 1,
      control: { mode: 'agent' },
    });
    await tick(service);
    assert.equal((await service.store.get('session', 's')).state, 'stopped');
    assert.equal(workerHealth.lastError, null);
    assert.ok(workerHealth.lastSuccess > 0);
  });

  it('keeps a failed tick’s error for the health check instead of throwing', async () => {
    const service = scratchService();
    mock.method(service.store, 'transact', async () => {
      throw new Error('storage down');
    });
    await tick(service);
    assert.equal(workerHealth.lastError, 'storage down');
    workerHealth.lastError = null;
  });

  it('counts pending cleanup for the health check', async () => {
    const service = scratchService();
    await putRow(service, 'session', 'c', {
      id: 'c',
      project: 'p',
      provider: 'oya-cloud',
      state: 'cleanup_pending',
      instance: 'other',
      leaseUntil: Date.now() + 60_000,
      fence: 1,
    });
    await tick(service);
    assert.equal(workerHealth.pendingCleanup, 1);
  });
});

describe('startWorkers', () => {
  it('starts the timers and advertises this replica, and stops them again', async () => {
    await startWorkers();
    await stopWorkers();
    assert.equal(flags.running, false);
  });

  it('refuses an unroutable cluster configuration at once', () => {
    const saved = process.env.OYA_INSTANCE_URL;
    process.env.OYA_INSTANCE_URL = 'ftp://replica';
    try {
      assert.throws(() => startWorkers(), /HTTP\(S\) URL and cluster secret/);
    } finally {
      restoreEnv('OYA_INSTANCE_URL', saved);
    }
  });
});
