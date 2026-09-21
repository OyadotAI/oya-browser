/**
 * Unit tests for provider cleanup: releasing a claimed session's resource by
 * its descriptor, closing a browser still connected here, marking the session
 * stopped, and backing off when the outcome is unknown or the release fails.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { projectId } from '../../../../../src/modules/control/service.ts';
import { runCleanup } from '../../../../../src/modules/control/worker/cleanup.ts';
import { workerHealth } from '../../../../../src/modules/control/worker/state.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { sealText } from '../../../../../src/platform/secrets.ts';
import { connectBrowser, disconnectBrowser } from '../../../support/fakes.ts';
import { json, stubFetch } from '../../../support/http.ts';
import { patchRow, putRow, scratchService } from '../../../support/control.ts';

const A = 'key-a',
  NOW = 1_700_000_000_000;
let service;
beforeEach(async () => {
  service = scratchService();
  await service.project(A);
});
afterEach(() => mock.restoreAll());

/** Stores a claimed cleanup job and returns it as the sweep hands it over. */
async function job(id, extra = {}) {
  await putRow(service, 'session', id, {
    id,
    project: projectId(A),
    state: 'cleanup_pending',
    provider: 'oya-cloud',
    fence: 2,
    cleanupLease: NOW + 120_000,
    ...extra,
  });
  return service.store.get('session', id);
}
const stored = (id) => service.store.get('session', id);

describe('runCleanup', () => {
  it('stops an attach-only session, which has no resource of ours', async () => {
    await runCleanup(service, [await job('s', { provider: 'cdp' })], NOW);
    const x = await stored('s');
    assert.deepEqual([x.state, x.cleanupLease], ['stopped', null]);
  });

  it('closes a browser still connected here and forgets it', async () => {
    const ws = connectBrowser('s', A);
    await runCleanup(service, [await job('s')], NOW);
    assert.deepEqual(ws.closed, { code: 4008, reason: 'Stopped by control plane' });
    assert.equal(registry.get('s'), undefined);
    assert.equal((await stored('s')).state, 'stopped');
    disconnectBrowser('s');
  });

  it('releases a vendor resource through its sealed release request', async () => {
    const calls = stubFetch(() => json({}, 200));
    const cleanup = {
      kind: 'vendor',
      sealed: sealText('provider-cleanup', { url: 'https://v.example/r', method: 'DELETE' }),
    };
    await runCleanup(service, [await job('s', { cleanup })], NOW);
    assert.equal(calls[0].url, 'https://v.example/r');
    assert.equal(calls[0].init.method, 'DELETE');
    assert.equal((await stored('s')).state, 'stopped');
  });

  it('backs off and records the error when the release fails', async () => {
    stubFetch(() => json({}, 500));
    const cleanup = { kind: 'vendor', sealed: sealText('provider-cleanup', { url: 'https://v.example/r' }) };
    await runCleanup(service, [await job('s', { cleanup, cleanupAttempts: 2 })], NOW);
    const x = await stored('s');
    assert.equal(x.state, 'cleanup_pending');
    assert.equal(x.cleanupAttempts, 3);
    assert.equal(x.nextCleanupAt, NOW + 4000);
    assert.equal(x.cleanupLease, null);
    assert.equal(x.cleanupError, 'Provider cleanup failed; retry pending');
  });

  it('refuses to guess about a provider resource with no descriptor, and retries later', async () => {
    await runCleanup(service, [await job('s')], NOW);
    assert.equal((await stored('s')).cleanupAttempts, 1);
  });

  it('skips a project whose key cannot be decrypted, noting it for health', async () => {
    await patchRow(service, 'project', projectId(A), { key: 'garbage' });
    const before = workerHealth.lastError;
    await runCleanup(service, [await job('s', { provider: 'cdp' })], NOW);
    assert.equal((await stored('s')).state, 'cleanup_pending');
    assert.match(workerHealth.lastError, /could not be decrypted/);
    workerHealth.lastError = before;
  });

  it('leaves a session alone once a newer claim has taken it', async () => {
    const claimed = await job('s', { provider: 'cdp' });
    await patchRow(service, 'session', 's', { fence: 3 });
    await runCleanup(service, [claimed], NOW);
    assert.equal((await stored('s')).state, 'cleanup_pending');
  });
});
