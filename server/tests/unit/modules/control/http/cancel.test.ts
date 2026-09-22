/**
 * Unit tests for POST /control/sessions/:id/cancel: a session in any state is
 * stopped and its final state answered, so what the caller is told is what the
 * fleet shows. A browser this replica holds is gone when the answer arrives.
 */
import { describe, it, before, after, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../../support/data-dir.ts';

ownDataDir('oya-control-cancel-');
const { controlRouter } = await import('../../../../../src/modules/control/routes.ts');
const { control } = await import('../../../../../src/modules/control/service.ts');
const { registry } = await import('../../../../../src/modules/browsers/registry.ts');
const { OPERATOR_CLOSE } = await import('../../../../../src/modules/browsers/constants.ts');
const { allowKey, callRoute } = await import('../../../support/agent.ts');
const { connectBrowser, disconnectBrowser } = await import('../../../support/fakes.ts');
const { driveBrowser } = await import('../../../support/browsers.ts');
const { readySession, patchRow } = await import('../../../support/control.ts');

const A = 'cancel-key-a',
  B = 'cancel-key-b';
let forget = [],
  n = 0,
  id;
before(() => (forget = [allowKey(A), allowKey(B)]));
after(() => forget.forEach((f) => f()));
beforeEach(async () => {
  id = `cx-${n++}`;
  await readySession(control(), A, id);
});
afterEach(() => {
  mock.restoreAll();
  disconnectBrowser(id);
});

/** Cancels `id` as `key`. */
const cancel = (key = A) => callRoute(controlRouter, { method: 'POST', url: `/sessions/${id}/cancel`, key });

describe('POST /control/sessions/:id/cancel', () => {
  it('cancel on a ready session removes the browser and answers stopped', async () => {
    const ws = connectBrowser(id, A);
    const res = await cancel();
    assert.deepEqual([res.status, res.body.state], [200, 'stopped']);
    assert.equal(registry.get(id), undefined);
    assert.equal(ws.closed?.code, OPERATOR_CLOSE);
  });

  it('cancel on a terminal session answers 200 with the session as it is, and writes no event', async () => {
    await patchRow(control(), 'session', id, { state: 'stopped' });
    const before = await control().session(A, id);
    const events = (await control().events(A)).length;
    const res = await cancel();
    assert.deepEqual([res.status, res.body], [200, before]);
    assert.equal((await control().events(A)).length, events);
  });

  it('cancel refuses to lose the profile and leaves the session ready', async () => {
    const engine: any = driveBrowser(id, () => ({ ok: true }), A);
    registry.get(id).persona = { id: 'p-cancel' };
    engine.cookies = async () => Promise.reject(new Error('target closed'));
    const res = await cancel();
    assert.equal(res.status, 409);
    assert.match(res.body.error, /Could not save profile before stopping/);
    assert.equal(res.body.code, 'cancel_failed');
    assert.equal((await control().session(A, id)).state, 'ready');
    assert.ok(registry.get(id));
  });

  it('cancel answers 502 and leaves cleanup pending when the vendor will not release the browser', async () => {
    driveBrowser(id, () => ({ ok: true }), A);
    registry.get(id).release = async () => Promise.reject(new Error('vendor says no'));
    mock.method(console, 'error', () => {});
    const res = await cancel();
    registry.get(id).release = null;
    assert.deepEqual([res.status, res.body.code], [502, 'cancel_failed']);
    assert.match(res.body.error, /vendor says no/);
    assert.equal((await control().session(A, id)).state, 'cleanup_pending');
    assert.ok(registry.get(id));
  });

  it('cancel on a session with no browser here still answers cleanup pending', async () => {
    const res = await cancel();
    assert.deepEqual([res.status, res.body.state], [200, 'cleanup_pending']);
  });

  it("cancel on another project's session is a 404 and leaves it running", async () => {
    const ws = connectBrowser(id, A);
    const res = await cancel(B);
    assert.equal(res.status, 404);
    assert.equal(ws.closed, null);
    assert.equal((await control().session(A, id)).state, 'ready');
  });
});
