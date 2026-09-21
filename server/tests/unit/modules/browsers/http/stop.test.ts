/**
 * Unit tests for the routes that take browsers off the fleet: stop one or
 * many, force a disconnect of one or of every browser on the key, and detach
 * a CDP browser.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { detach, disconnect, disconnectAll, stopMany, stopOne } from '../../../../../src/modules/browsers/http/stop.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { sessions as gatewaySessions } from '../../../../../src/modules/gateway/service.ts';
import { fingerprint, recent } from '../../../../../src/platform/audit.ts';
import { connectBrowser, disconnectBrowser } from '../../../support/fakes.ts';
import { FakeResponse, fakeRequest, stubControl } from '../../../support/browsers.ts';

const IDS = ['s-1', 's-2', 's-3'];

/** Calls `handler` as `key` with `params` and `body`; returns the response. */
async function call(handler, { key = 'k-stop', params = {}, body = {} } = {}) {
  const res = new FakeResponse();
  await handler(fakeRequest({ key, params, body }), res);
  return res;
}

describe('stop routes', () => {
  beforeEach(() => stubControl());
  afterEach(() => {
    mock.restoreAll();
    IDS.forEach(disconnectBrowser);
    gatewaySessions.delete('g-1');
  });

  it('stops one browser and answers 200', async () => {
    connectBrowser('s-1', 'k-stop');
    const res = await call(stopOne, { params: { browserId: 's-1' } });
    assert.deepEqual([res.statusCode, res.body.ok], [200, true]);
  });

  it('answers 404 for a browser it cannot stop', async () => {
    const res = await call(stopOne, { params: { browserId: 'nope' } });
    assert.deepEqual([res.statusCode, res.body.error], [404, 'Browser not connected']);
  });

  it('answers a failed stop with the status the failure carries', async () => {
    connectBrowser('s-1', 'k-stop');
    registry.get('s-1').release = async () => Promise.reject(new Error('no'));
    const res = await call(stopOne, { params: { browserId: 's-1' } });
    assert.equal(res.statusCode, 502);
    registry.get('s-1').release = null;
  });

  it('stops the listed ids, each reporting separately', async () => {
    connectBrowser('s-1', 'k-stop');
    const res = await call(stopMany, { body: { ids: ['s-1', 'missing'] } });
    assert.equal(res.body.stopped, 1);
    assert.deepEqual(
      res.body.results.map((r) => r.ok),
      [true, false],
    );
  });

  it('requires ids or all', async () => {
    const res = await call(stopMany, { body: { ids: 'not-a-list' } });
    assert.deepEqual([res.statusCode, res.body], [400, { error: 'Pass ids: [...] or all: true' }]);
  });

  it('stops every browser, gateway session and durable session the key has with all', async () => {
    stubControl({ sessions: async () => [{ id: 's-3' }] });
    connectBrowser('s-1', 'k-stop');
    connectBrowser('s-2', 'k-other');
    gatewaySessions.set('g-1', { id: 'g-1', apiKey: 'k-stop', destroy: async () => {} } as any);
    const res = await call(stopMany, { body: { all: true } });
    assert.deepEqual(res.body.results.map((r) => r.id).sort(), ['g-1', 's-1', 's-3']);
    assert.equal(registry.isConnected('s-2'), true);
  });

  it('answers an empty success for all when the key has nothing', async () => {
    const res = await call(stopMany, { key: 'k-nothing', body: { all: true } });
    assert.deepEqual(res.body, { ok: true, stopped: 0, results: [] });
  });

  it('forces one browser off and audits the reason', async () => {
    const ws = connectBrowser('s-1', 'k-stop');
    const res = await call(disconnect, { params: { browserId: 's-1' }, body: { reason: 'runaway' } });
    assert.deepEqual(res.body, { ok: true, disconnected: 's-1' });
    assert.equal(ws.closed.reason, 'Disconnected by operator');
    assert.equal(recent({ action: 'browser.disconnect' })[0].meta.reason, 'runaway');
  });

  it('will not disconnect another key’s browser', async () => {
    connectBrowser('s-1', 'k-other');
    const res = await call(disconnect, { params: { browserId: 's-1' } });
    assert.equal(res.statusCode, 404);
    assert.equal(registry.isConnected('s-1'), true);
  });

  it('drops every browser on the calling key only, auditing the key by fingerprint', async () => {
    connectBrowser('s-1', 'k-stop');
    connectBrowser('s-2', 'k-stop');
    connectBrowser('s-3', 'k-other');
    const res = await call(disconnectAll);
    assert.deepEqual(res.body, { ok: true, disconnected: 2 });
    assert.equal(registry.isConnected('s-3'), true);
    assert.equal(recent({ action: 'key.disconnect' })[0].target_id, fingerprint('k-stop'));
  });

  it('detaches a browser and audits it', async () => {
    connectBrowser('s-1', 'k-stop');
    const res = await call(detach, { params: { browserId: 's-1' } });
    assert.deepEqual(res.body, { ok: true });
    assert.equal(registry.isConnected('s-1'), false);
  });

  it('answers 404 when detaching a browser that is not the caller’s', async () => {
    const res = await call(detach, { params: { browserId: 'nope' } });
    assert.deepEqual([res.statusCode, res.body], [404, { error: 'Browser not found' }]);
  });

  it('answers a failed detach with its status', async () => {
    connectBrowser('s-1', 'k-stop');
    registry.get('s-1').release = async () => Promise.reject(new Error('no'));
    const res = await call(detach, { params: { browserId: 's-1' } });
    assert.equal(res.statusCode, 502);
    registry.get('s-1').release = null;
  });
});
