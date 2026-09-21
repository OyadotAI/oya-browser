/**
 * Unit tests for starting a browser through the one start endpoint: persona
 * and quota checks, picking the launcher, the CDP launcher end to end (with
 * the driver's connect stubbed), the Oya Cloud answer when it is not
 * configured, and failures answered with their status or 502.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { startBrowser } from '../../../../../src/modules/browsers/lifecycle/start.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { CDPDriver } from '../../../../../src/drivers/cdp.ts';
import { isConfigured as sandboxConfigured } from '../../../../../src/drivers/sandbox.ts';
import { container } from '../../../../../src/app/container.ts';
import { QUOTAS } from '../../../../../src/platform/limits.ts';
import { recent } from '../../../../../src/platform/audit.ts';
import { HttpError } from '../../../../../src/platform/errors.ts';
import { connectBrowser, disconnectBrowser } from '../../../support/fakes.ts';
import { FakeResponse, fakeRequest, stubControl } from '../../../support/browsers.ts';

const KEY = 'start-key';
const B = 'b-start';

/** A start request for session B with `body`. */
const startReq = (body: object) =>
  fakeRequest({ key: KEY, body, headers: { host: 'oya.test' }, extra: { controlSession: { id: B } } });

/** Runs startBrowser and returns the response. */
async function start(body: object) {
  const res = new FakeResponse();
  await startBrowser(startReq(body), res);
  return res;
}

describe('startBrowser', () => {
  beforeEach(() => stubControl());
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('answers 404 for a persona the key does not own', async () => {
    const res = await start({ provider: 'cdp', persona: 'p-nobody' });
    assert.equal(res.statusCode, 404);
    assert.match(res.body.error, /No such persona/);
  });

  it('answers 400 when resolving the persona fails without a status', async () => {
    mock.method(container.personas, 'resolve', () => {
      throw new Error('broken');
    });
    const res = await start({ provider: 'cdp' });
    assert.deepEqual([res.statusCode, res.body.error], [400, 'broken']);
  });

  it('answers 429 once the key holds its browser quota', async () => {
    const saved = QUOTAS.browsers;
    QUOTAS.browsers = 1;
    connectBrowser(B, KEY);
    try {
      const res = await start({ provider: 'cdp' });
      assert.equal(res.statusCode, 429);
      assert.match(res.body.error, /Browser quota reached/);
    } finally {
      QUOTAS.browsers = saved;
    }
  });

  it('answers with the control plane’s status when provisioning may not go on, and audits it', async () => {
    stubControl({
      assertProvisioning: async () => {
        throw new HttpError(409, 'Session was cancelled');
      },
    });
    const res = await start({ provider: 'cdp' });
    assert.deepEqual([res.statusCode, res.body.error], [409, 'Session was cancelled']);
    assert.equal(recent({ action: 'browser.start', outcome: 'error' })[0].meta.error, 'Session was cancelled');
  });

  it('answers 502 when a launcher fails without a status', async () => {
    mock.method(CDPDriver.prototype, 'connect', async () => {
      throw new Error('socket hang up');
    });
    const res = await start({ provider: 'cdp', wsUrl: 'wss://8.8.8.8/devtools' });
    assert.deepEqual([res.statusCode, res.body.error], [502, 'socket hang up']);
    assert.equal(registry.isConnected(B), false);
  });

  it('frees the persona’s slot when the driver cannot connect', async () => {
    mock.method(CDPDriver.prototype, 'connect', async () => {
      throw new Error('refused');
    });
    const released = mock.method(container.personas, 'release');
    await start({ provider: 'cdp', wsUrl: 'wss://8.8.8.8/devtools' });
    assert.equal(released.mock.callCount(), 1);
  });

  it('hands the vendor session back when the persona is at its cap', async () => {
    mock.method(container.personas, 'acquire', () => {
      throw new HttpError(429, 'Persona at its concurrency cap');
    });
    const res = await start({ provider: 'cdp', wsUrl: 'wss://8.8.8.8/devtools' });
    assert.deepEqual([res.statusCode, res.body.error], [429, 'Persona at its concurrency cap']);
  });

  it('hands over the browser already connected when nothing is configured to start one', async () => {
    connectBrowser('desktop-1', KEY);
    const res = await start({});
    assert.equal(res.body.id, 'desktop-1');
    assert.equal(res.body.status, 'ready');
    assert.equal(res.body.reused, true);
    assert.equal(recent({ action: 'browser.start', outcome: 'ok' })[0].meta.reused, true);
    disconnectBrowser('desktop-1');
  });

  it('never hands over another key’s browser', async () => {
    connectBrowser('someone-elses', 'other-key');
    const res = await start({});
    assert.equal(res.statusCode, 409);
    assert.match(res.body.error, /no browser is connected/);
    disconnectBrowser('someone-elses');
  });

  it('says how to get a browser when the key can start none and has none', async () => {
    const res = await start({});
    assert.equal(res.statusCode, 409);
    assert.match(res.body.error, /desktop browser/);
    assert.match(res.body.error, /npx @oya-ai\/cli init/);
    assert.match(res.body.error, /remote-debugging-port/);
  });

  it('starts a CDP browser as the persona and answers 201 with the gateway URL', async () => {
    mock.method(CDPDriver.prototype, 'connect', async function () {
      return this;
    });
    const res = await start({ provider: 'cdp', wsUrl: 'wss://8.8.8.8/devtools', name: 'Agent' });
    assert.equal(res.statusCode, 201);
    assert.equal(res.body.id, B);
    assert.equal(res.body.status, 'ready');
    assert.equal(res.body.cdpUrl, `ws://oya.test/connect?ticket=ticket-1&browser=${B}`);
    const b = registry.get(B);
    assert.deepEqual([b.name, b.clientType, b.persona.id], ['Agent', 'cdp', res.body.persona]);
    assert.equal(recent({ action: 'browser.start', outcome: 'ok' })[0].target_id, B);
  });

  it('frees the persona’s slot when a started CDP browser is removed', async () => {
    mock.method(CDPDriver.prototype, 'connect', async function () {
      return this;
    });
    await start({ provider: 'cdp', wsUrl: 'wss://8.8.8.8/devtools' });
    const persona = registry.get(B).persona;
    assert.equal(container.personas.activeCount(persona.id), 1);
    registry.remove(B);
    await new Promise((r) => setImmediate(r));
    assert.equal(container.personas.activeCount(persona.id), 0);
  });

  it(
    'answers 409 naming the missing settings when Oya Cloud is not configured',
    { skip: sandboxConfigured() },
    async () => {
      const res = await start({ provider: 'oya-cloud' });
      assert.equal(res.statusCode, 409);
      assert.ok(res.body.missing.includes('OYA_PUBLIC_WS_URL'));
      assert.match(res.body.error, /^Cloud browsers need .+, which are not set\. OYA_PUBLIC_WS_URL must be reachable/);
    },
  );
});
