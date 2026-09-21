/**
 * Unit tests for a session's life after admission: cancel, the provisioning
 * outcome, the provisioning lease, worker patches under a fence, and adopting
 * a browser that connected to this replica. Another project's sessions answer
 * 404 and are left untouched.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { instanceId } from '../../../../../src/modules/control/service.ts';
import { patchRow, readySession, scratchService } from '../../../support/control.ts';

const A = 'key-a',
  B = 'key-b',
  NOW = 1_700_000_000_000;
let service;
beforeEach(() => {
  mock.timers.enable({ apis: ['Date'], now: NOW });
  service = scratchService();
});
afterEach(() => mock.timers.reset());

/** The stored session. */
const stored = (id) => service.store.get('session', id);

describe('cancel', () => {
  it('stops a queued session outright', async () => {
    await service.reserve(A, { id: 'a', provider: 'cdp', maxConcurrent: 1 });
    await service.reserve(A, { id: 'q', provider: 'cdp', maxConcurrent: 1, request: { queueMs: 1000 } });
    assert.equal((await service.cancel(A, 'q')).state, 'stopped');
  });

  it('sends a live session to cleanup, recording why', async () => {
    await service.reserve(A, { id: 's', provider: 'cdp' });
    const x = await service.cancel(A, 's');
    assert.equal(x.state, 'cleanup_pending');
    assert.deepEqual((await service.events(A)).at(-1).detail, { reason: 'cancelled' });
  });

  it('with force, reconciles a session with nothing to delete straight to stopped', async () => {
    await readySession(service, A, 's');
    assert.equal((await service.cancel(A, 's', { force: true })).state, 'stopped');
    assert.deepEqual((await service.events(A)).at(-1).detail, { reason: 'reconciled' });
  });

  it('with force, still cleans up a session that has a deletion descriptor or a creation in flight', async () => {
    await service.reserve(A, { id: 'c', provider: 'cdp', cleanup: { kind: 'vendor' } });
    await service.reserve(A, { id: 'p', provider: 'cdp' });
    assert.equal((await service.cancel(A, 'c', { force: true })).state, 'cleanup_pending');
    assert.equal((await service.cancel(A, 'p', { force: true })).state, 'cleanup_pending');
  });

  it('leaves a terminal session as it is', async () => {
    await service.reserve(A, { id: 's', provider: 'cdp' });
    await service.complete(A, 's', 400, {});
    assert.equal((await service.cancel(A, 's')).state, 'failed');
  });

  it('answers 404 for another project’s session and leaves it running', async () => {
    await readySession(service, B, 'theirs');
    await assert.rejects(service.cancel(A, 'theirs'), { status: 404 });
    assert.equal((await stored('theirs')).state, 'ready');
  });
});

describe('complete', () => {
  beforeEach(() => service.reserve(A, { id: 's', provider: 'oya-cloud' }));

  it('marks a started browser ready and keeps the response private', async () => {
    const x = await service.complete(A, 's', 200, { cdpUrl: 'ws://x' });
    assert.equal(x.state, 'ready');
    assert.equal('response' in x, false);
    assert.deepEqual((await stored('s')).response, { status: 200, body: { cdpUrl: 'ws://x' } });
    assert.equal((await stored('s')).provisioningActive, false);
  });

  it('keeps provisioning while the browser says it is still starting', async () => {
    assert.equal((await service.complete(A, 's', 202, { status: 'starting' })).state, 'provisioning');
  });

  it('fails a refused request, and marks a provider error’s outcome unknown', async () => {
    assert.equal((await service.complete(A, 's', 400, {})).state, 'failed');
    await service.reserve(A, { id: 't', provider: 'oya-cloud' });
    assert.equal((await service.complete(A, 't', 500, {})).state, 'unknown_outcome');
  });

  it('fails an attach-only provider’s error, since nothing of ours can exist', async () => {
    await service.reserve(A, { id: 'c', provider: 'cdp' });
    assert.equal((await service.complete(A, 'c', 502, {})).state, 'failed');
  });

  it('sends a failed start with a deletion descriptor to cleanup', async () => {
    await service.reserve(A, { id: 'd', provider: 'oya-cloud', cleanup: { kind: 'sandbox' } });
    assert.equal((await service.complete(A, 'd', 500, {})).state, 'cleanup_pending');
  });

  it('never moves a session that is already stopping', async () => {
    await service.cancel(A, 's');
    assert.equal((await service.complete(A, 's', 200, {})).state, 'cleanup_pending');
  });
});

describe('assertProvisioning', () => {
  it('passes while this replica holds a live lease', async () => {
    await service.reserve(A, { id: 's', provider: 'cdp' });
    await service.assertProvisioning(A, 's');
  });

  it('refuses a cancelled, unknown or foreign session as creation_cancelled', async () => {
    await service.reserve(A, { id: 's', provider: 'cdp' });
    await service.cancel(A, 's');
    await assert.rejects(service.assertProvisioning(A, 's'), { code: 'creation_cancelled' });
    await assert.rejects(service.assertProvisioning(A, 'missing'), { code: 'creation_cancelled' });
    await service.reserve(B, { id: 'theirs', provider: 'cdp' });
    await assert.rejects(service.assertProvisioning(A, 'theirs'), { code: 'creation_cancelled' });
  });

  it('refuses once the lease lapses or another replica holds it', async () => {
    await service.reserve(A, { id: 's', provider: 'cdp' });
    mock.timers.tick(180_001);
    await assert.rejects(service.assertProvisioning(A, 's'), { code: 'stale_worker' });
    await patchRow(service, 'session', 's', { leaseUntil: NOW * 2, instance: 'other' });
    await assert.rejects(service.assertProvisioning(A, 's'), { code: 'stale_worker' });
  });
});

describe('update', () => {
  it('patches the session and announces a state change', async () => {
    await service.reserve(A, { id: 's', provider: 'cdp' });
    const x = await service.update(A, 's', { state: 'ready', note: 1 });
    assert.equal(x.note, 1);
    assert.equal((await service.events(A)).at(-1).type, 'session.ready');
  });

  it('refuses a worker holding a stale fence', async () => {
    await service.reserve(A, { id: 's', provider: 'cdp' });
    await assert.rejects(service.update(A, 's', { note: 1 }, { fence: 2 }), { code: 'stale_worker' });
  });

  it('refuses to move a terminal session to another state', async () => {
    await service.reserve(A, { id: 's', provider: 'cdp' });
    await service.complete(A, 's', 400, {});
    await assert.rejects(service.update(A, 's', { state: 'ready' }), { code: 'terminal_session' });
    await service.update(A, 's', { note: 1 });
  });

  it('lets a stopping session only move on to stopped', async () => {
    await service.reserve(A, { id: 's', provider: 'cdp' });
    await service.cancel(A, 's');
    await assert.rejects(service.update(A, 's', { state: 'ready' }), { code: 'session_stopping' });
    assert.equal((await service.update(A, 's', { state: 'stopped' })).state, 'stopped');
  });

  it('answers 404 for another project’s session', async () => {
    await service.reserve(B, { id: 'theirs', provider: 'cdp' });
    await assert.rejects(service.update(A, 'theirs', { state: 'stopped' }), { status: 404 });
  });
});

describe('adopt', () => {
  it('reserves a browser that connected without one, and marks it ready here', async () => {
    const x = await service.adopt(A, { id: 'b', provider: 'oya-desktop', persona: 'p' });
    assert.equal(x.state, 'ready');
    assert.equal(x.instance, instanceId);
    assert.equal(x.leaseUntil, NOW + 30_000);
    assert.equal(x.fence, 2);
    assert.equal(x.persona, 'p');
  });

  it('marks a provisioning session ready', async () => {
    await service.reserve(A, { id: 'b', provider: 'oya-cloud' });
    assert.equal((await service.adopt(A, { id: 'b', provider: 'oya-cloud' })).state, 'ready');
  });

  it('refuses a stopped or stopping session', async () => {
    await service.reserve(A, { id: 'b', provider: 'cdp' });
    await service.cancel(A, 'b');
    await assert.rejects(service.adopt(A, { id: 'b', provider: 'cdp' }), { code: 'session_stopped' });
  });

  it('refuses a ready session another replica still holds', async () => {
    await readySession(service, A, 'b');
    await patchRow(service, 'session', 'b', { instance: 'other', leaseUntil: NOW + 10_000 });
    await assert.rejects(service.adopt(A, { id: 'b', provider: 'cdp' }), { code: 'session_owned' });
  });

  it('takes a slot again for a disconnected session, refusing at capacity', async () => {
    await readySession(service, A, 'b');
    await patchRow(service, 'session', 'b', { state: 'disconnected' });
    await readySession(service, A, 'other');
    await assert.rejects(service.adopt(A, { id: 'b', provider: 'cdp', maxConcurrent: 1 }), {
      code: 'quota_exceeded',
    });
    const x = await service.adopt(A, { id: 'b', provider: 'cdp', maxConcurrent: 2 });
    assert.equal(x.state, 'ready');
    assert.equal(x.meteredAt, NOW);
  });

  it('takes the connecting provider for an imported legacy session', async () => {
    await readySession(service, A, 'b');
    await patchRow(service, 'session', 'b', { provider: 'legacy' });
    assert.equal((await service.adopt(A, { id: 'b', provider: 'oya-desktop' })).provider, 'oya-desktop');
  });

  it('answers 404 for another project’s session', async () => {
    await readySession(service, B, 'theirs');
    await assert.rejects(service.adopt(A, { id: 'theirs', provider: 'cdp' }), { status: 404 });
  });
});
