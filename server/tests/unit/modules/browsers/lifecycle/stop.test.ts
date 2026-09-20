/**
 * Unit tests for stopping one browser, whatever it is: a browser held here
 * (its profile saved, its vendor session released, its socket dropped), a
 * durable or gateway session held elsewhere, and a browser the caller cannot
 * see.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { stopBrowser } from '../../../../../src/modules/browsers/lifecycle/stop.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { sessions as gatewaySessions } from '../../../../../src/modules/gateway/service.ts';
import { getAll as getAllCookies } from '../../../../../src/modules/personas/cookies.ts';
import { recent } from '../../../../../src/platform/audit.ts';
import { OPERATOR_CLOSE } from '../../../../../src/modules/browsers/constants.ts';
import { connectBrowser, disconnectBrowser } from '../../../support/fakes.ts';
import { driveBrowser, fakeRequest, stubControl } from '../../../support/browsers.ts';

const B = 'b-stop';
const req = (key = 'key-a') => fakeRequest({ key });

describe('stopBrowser: a browser held here', () => {
  beforeEach(() => stubControl());
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('drops the socket and reports the browser stopped', async () => {
    const ws = connectBrowser(B);
    const result = await stopBrowser(req(), B);
    assert.deepEqual(result, { id: B, ok: true, sandboxRemoved: null, provider: null });
    assert.deepEqual(ws.closed, { code: OPERATOR_CLOSE, reason: 'Stopped by operator' });
    assert.equal(registry.isConnected(B), false);
    assert.equal(recent({ action: 'browser.stop' })[0].target_id, B);
  });

  it('refuses a browser that belongs to another key, as if it were not there', async () => {
    const ws = connectBrowser(B, 'key-a');
    assert.deepEqual(await stopBrowser(req('key-b'), B), { id: B, ok: false, error: 'Browser not connected' });
    assert.equal(ws.closed, null);
  });

  it('cancels a managed runtime session and leaves cleanup to the runtime', async () => {
    const { cancel } = stubControl({ findSession: async () => ({ runtime: 'docker', state: 'ready' }) });
    connectBrowser(B);
    assert.deepEqual(await stopBrowser(req(), B), { id: B, ok: true, status: 'cleanup_pending' });
    assert.equal(cancel.mock.callCount(), 1);
  });

  it('marks a durable session cleanup_pending, then stopped', async () => {
    const { update } = stubControl({ findSession: async () => ({ state: 'ready' }) });
    connectBrowser(B);
    await stopBrowser(req(), B);
    assert.deepEqual(
      update.mock.calls.map((c) => c.arguments[2].state),
      ['cleanup_pending', 'stopped'],
    );
  });

  it('reports a sandbox that could not be removed and leaves the durable session in cleanup', async () => {
    const { update } = stubControl({ findSession: async () => ({ state: 'ready' }) });
    mock.method(console, 'warn', () => {});
    connectBrowser(B);
    const result = await stopBrowser(req(), B, { sandbox: true });
    assert.equal(result.sandboxRemoved, false);
    assert.deepEqual(
      update.mock.calls.map((c) => c.arguments[2].state),
      ['cleanup_pending'],
    );
  });

  it('keeps the browser and answers 502 when the vendor will not release it', async () => {
    connectBrowser(B);
    registry.get(B).release = async () => Promise.reject(new Error('vendor says no'));
    const result = await stopBrowser(req(), B);
    assert.deepEqual(result, { id: B, ok: false, status: 502, error: 'vendor says no' });
    assert.equal(registry.isConnected(B), true);
    registry.get(B).release = null;
  });

  it('releases the vendor session once, then drops the browser', async () => {
    connectBrowser(B);
    const release = mock.fn(async () => {});
    registry.get(B).release = release;
    assert.equal((await stopBrowser(req(), B)).ok, true);
    assert.equal(release.mock.callCount(), 1);
  });

  it('saves a driven browser’s cookies into its persona before stopping', async () => {
    const driver: any = driveBrowser(B, () => ({ ok: true }));
    registry.get(B).persona = { id: 'p-stop-save' };
    const cookie = { name: 'sid', value: 'v', domain: 'example.com', path: '/' };
    driver.conn = { send: async () => ({ cookies: [cookie] }) };
    assert.equal((await stopBrowser(req(), B)).ok, true);
    assert.ok(getAllCookies('p-stop-save').some((c) => c.name === 'sid'));
  });

  it('refuses to stop when the profile cannot be saved', async () => {
    const driver: any = driveBrowser(B, () => ({ ok: true }));
    registry.get(B).persona = { id: 'p-stop-fail' };
    driver.conn = { send: async () => Promise.reject(new Error('target closed')) };
    const result = await stopBrowser(req(), B);
    assert.deepEqual(result, { id: B, ok: false, error: 'Could not save profile before stopping: target closed' });
    assert.equal(registry.isConnected(B), true);
  });

  it('stops anyway with force, auditing the lost profile', async () => {
    const driver: any = driveBrowser(B, () => ({ ok: true }));
    registry.get(B).persona = { id: 'p-stop-force' };
    driver.conn = { send: async () => Promise.reject(new Error('target closed')) };
    assert.equal((await stopBrowser(req(), B, { force: true })).ok, true);
    assert.equal(recent({ action: 'profile.capture.failed' })[0].target_id, B);
  });
});

describe('stopBrowser: a browser not held here', () => {
  afterEach(() => {
    mock.restoreAll();
    gatewaySessions.delete(B);
  });

  it('ends the caller’s own gateway session', async () => {
    stubControl();
    const destroy = mock.fn(async () => {});
    gatewaySessions.set(B, { apiKey: 'key-a', destroy } as any);
    assert.deepEqual(await stopBrowser(req(), B), { id: B, ok: true });
    assert.equal(destroy.mock.callCount(), 1);
  });

  it('leaves another key’s gateway session alone', async () => {
    stubControl();
    const destroy = mock.fn(async () => {});
    gatewaySessions.set(B, { apiKey: 'key-b', destroy } as any);
    assert.equal((await stopBrowser(req(), B)).ok, false);
    assert.equal(destroy.mock.callCount(), 0);
  });

  it('reports a durable session that is already over without cancelling it', async () => {
    const { cancel } = stubControl({ findSession: async () => ({ state: 'stopped' }) });
    assert.deepEqual(await stopBrowser(req(), B), { id: B, ok: true, status: 'stopped' });
    assert.equal(cancel.mock.callCount(), 0);
  });

  it('cancels a live durable session, passing force through', async () => {
    const { cancel } = stubControl({
      findSession: async () => ({ state: 'queued' }),
      cancel: async () => ({ state: 'cancelled' }),
    });
    assert.deepEqual(await stopBrowser(req(), B, { force: true }), { id: B, ok: true, status: 'cancelled' });
    assert.deepEqual(cancel.mock.calls[0].arguments, ['key-a', B, { force: true }]);
  });

  it('answers not connected when there is nothing by that id', async () => {
    stubControl();
    assert.deepEqual(await stopBrowser(req(), 'b-nowhere'), {
      id: 'b-nowhere',
      ok: false,
      error: 'Browser not connected',
    });
  });
});
