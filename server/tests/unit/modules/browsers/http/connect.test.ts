/**
 * Unit tests for attaching a caller's CDP browser (the driver's connect is
 * stubbed): answered 201 once registered, 429 over quota, and the failure
 * with the vendor session handed back.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { connectCdp } from '../../../../../src/modules/browsers/http/connect.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { CDPDriver } from '../../../../../src/drivers/cdp.ts';
import { QUOTAS } from '../../../../../src/platform/limits.ts';
import { recent } from '../../../../../src/platform/audit.ts';
import { connectBrowser, disconnectBrowser } from '../../../support/fakes.ts';
import { FakeResponse, fakeRequest, stubControl } from '../../../support/browsers.ts';

const B = 'b-connect';

/** Attaches with `body` as key k-connect and returns the response. */
async function attach(body: object) {
  const res = new FakeResponse();
  await connectCdp(fakeRequest({ key: 'k-connect', body, extra: { controlSession: { id: B } } }), res);
  return res;
}

describe('connectCdp', () => {
  beforeEach(() => stubControl());
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('registers the browser and answers 201', async () => {
    mock.method(CDPDriver.prototype, 'connect', async function () {
      return this;
    });
    const res = await attach({ wsUrl: 'wss://8.8.8.8/devtools', name: 'Mine' });
    assert.deepEqual([res.statusCode, res.body], [201, { id: B, provider: 'cdp', clientType: 'cdp' }]);
    assert.equal(registry.get(B).name, 'Mine');
    assert.equal(recent({ action: 'browser.connect' })[0].outcome, 'ok');
  });

  it('answers 429 and audits the denial over the browser quota', async () => {
    const saved = QUOTAS.browsers;
    QUOTAS.browsers = 1;
    connectBrowser(B, 'k-connect');
    try {
      const res = await attach({ wsUrl: 'wss://8.8.8.8/' });
      assert.equal(res.statusCode, 429);
      assert.equal(recent({ action: 'browser.connect' })[0].outcome, 'denied');
    } finally {
      QUOTAS.browsers = saved;
    }
  });

  it('answers the acquire failure’s status when the URL is refused', async () => {
    const res = await attach({});
    assert.deepEqual([res.statusCode, res.body.error], [400, 'wsUrl is required for the cdp provider']);
  });

  it('answers 502 and hands the session back when the driver cannot attach', async () => {
    mock.method(CDPDriver.prototype, 'connect', async () => {
      throw new Error('handshake failed');
    });
    const res = await attach({ wsUrl: 'wss://8.8.8.8/' });
    assert.deepEqual([res.statusCode, res.body.error], [502, 'Could not attach to the CDP browser: handshake failed']);
    assert.equal(registry.isConnected(B), false);
  });
});
