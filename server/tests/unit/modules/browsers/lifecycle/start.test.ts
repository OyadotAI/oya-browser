/**
 * Unit tests for starting a browser through the one start endpoint: persona
 * and quota checks, picking the launcher, the CDP launcher end to end (with
 * the driver's connect stubbed), the Oya Cloud answer when it is not
 * configured, and failures answered with their status or 502.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { startBrowser } from '../../../../../src/modules/browsers/lifecycle/start.ts';
import { stopBrowser } from '../../../../../src/modules/browsers/lifecycle/stop.ts';
import * as usage from '../../../../../src/platform/usage.ts';
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

/** A second browser on the same key: what a duplicate start becomes. */
const B2 = 'b-start-two';

/** The one Chrome two starts may fight over. */
const CHROME = 'wss://8.8.8.8/devtools/browser/one';

/** Starts session `id` on CHROME and returns the response. */
async function startOn(id: string) {
  const res = new FakeResponse();
  const req = fakeRequest({ key: KEY, body: { provider: 'cdp', wsUrl: CHROME }, extra: { controlSession: { id } } });
  await startBrowser(req, res);
  return res;
}

/** Lets the driver connect without a Chrome. */
const connectable = () =>
  mock.method(CDPDriver.prototype, 'connect', async function () {
    return this;
  });

/** Throws the way an unexpected bug would. */
function fail(): never {
  throw new Error('boom after connect');
}

describe('startBrowser', () => {
  beforeEach(() => stubControl());
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
    disconnectBrowser(B2);
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
      assert.deepEqual([res.statusCode, res.body.code], [429, 'quota_exceeded']);
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
    assert.deepEqual(
      [res.statusCode, res.body.error, res.body.code],
      [409, 'Session was cancelled', 'operation_failed'],
    );
    assert.equal(recent({ action: 'browser.start', outcome: 'error' })[0].meta.error, 'Session was cancelled');
  });

  it('answers 502 when a launcher fails without a status', async () => {
    mock.method(CDPDriver.prototype, 'connect', async () => {
      throw new Error('socket hang up');
    });
    const res = await start({ provider: 'cdp', wsUrl: 'wss://8.8.8.8/devtools' });
    assert.deepEqual([res.statusCode, res.body.error, res.body.code], [502, 'socket hang up', 'provider_failed']);
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

  it('asks for a wsUrl when the caller names cdp, even with a browser connected', async () => {
    connectBrowser('desktop-1', KEY);
    const res = await start({ provider: 'cdp' });
    assert.equal(res.statusCode, 400);
    assert.match(res.body.error, /wsUrl/);
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

  it('answers 409 naming the holder when a second start names a driven wsUrl', async () => {
    connectable();
    assert.equal((await startOn(B)).statusCode, 201);
    const res = await startOn(B2);
    assert.deepEqual(
      [res.statusCode, res.body.code, res.body.browserId],
      [409, 'endpoint_in_use', B],
      JSON.stringify(res.body),
    );
    assert.equal(
      res.body.error,
      `The Chrome at 8.8.8.8 is already held by browser ${B}. Stop that browser, or drive it.`,
    );
    assert.deepEqual([registry.isConnected(B), registry.isConnected(B2)], [true, false]);
  });

  it('two starts racing on one wsUrl: one 201, one 409', async () => {
    let open: () => void = () => {};
    const dialled = new Promise<void>((resolve) => (open = resolve));
    mock.method(CDPDriver.prototype, 'connect', async function () {
      await dialled;
      return this;
    });
    const both = Promise.all([startOn(B), startOn(B2)]);
    open();
    const codes = (await both).map((r) => r.statusCode).sort();
    assert.deepEqual(codes, [201, 409]);
    assert.equal([B, B2].filter((id) => registry.isConnected(id)).length, 1);
  });

  it('a stopped browser\u2019s wsUrl can be started again', async () => {
    connectable();
    assert.equal((await startOn(B)).statusCode, 201);
    // force: the stubbed driver has no Chrome to pull a profile from.
    assert.equal((await stopBrowser(startReq({}), B, { force: true })).ok, true);
    assert.equal((await startOn(B2)).statusCode, 201);
    // The browser's own socket closing (Chrome died) frees it too: the release runs a tick after the record goes.
    registry.get(B2).driver.engine.onClose();
    assert.equal(registry.isConnected(B2), false);
    await Promise.resolve();
    assert.equal((await startOn(B)).statusCode, 201);
  });

  it('a provisioning refusal after the session was acquired frees the endpoint', async () => {
    connectable();
    let asked = 0;
    stubControl({
      assertProvisioning: async () => {
        if (++asked === 2) throw new HttpError(409, 'Session was cancelled');
      },
    });
    assert.equal((await startOn(B)).statusCode, 409);
    assert.equal((await startOn(B)).statusCode, 201);
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

  it('answers 400 for a name that is not a string, before a session, a slot or a driver is taken', async () => {
    const connect = mock.method(CDPDriver.prototype, 'connect', async function () {
      return this;
    });
    const acquired = mock.method(container.personas, 'acquire');
    const res = await start({ provider: 'cdp', wsUrl: 'wss://8.8.8.8/devtools', name: { a: 1 } });
    assert.deepEqual([res.statusCode, res.body.error], [400, 'name must be a string, not an object']);
    assert.deepEqual([connect.mock.callCount(), acquired.mock.callCount()], [0, 0]);
  });

  it('a start that fails after its driver connected frees the slot, closes the driver and leaves no browser behind', async () => {
    const closed = mock.fn();
    mock.method(CDPDriver.prototype, 'connect', async function () {
      this.close = closed;
      return this;
    });
    const persona = container.personas.resolve(KEY);
    for (const step of ['before', 'after'] as const) {
      const breaking = step === 'before' ? mock.method(registry, 'add', () => fail()) : null;
      if (step === 'after') stubControl({ ticket: async () => fail() });
      const res = await start({ provider: 'cdp', wsUrl: 'wss://8.8.8.8/devtools' });
      breaking?.mock.restore();
      assert.equal(res.statusCode, 502, step);
      assert.equal(container.personas.activeCount(persona.id), 0, `slot still held (${step} registration)`);
      assert.equal(registry.isConnected(B), false, step);
    }
    assert.equal(closed.mock.callCount(), 2);
  });

  it('a failed start frees the slot and stops the usage clock even when the vendor refuses to take the session back', async () => {
    mock.method(CDPDriver.prototype, 'connect', async function () {
      return this;
    });
    // Steel over a faked network: the session is created, and its release answers 500.
    mock.method(globalThis, 'fetch', async (url: string) =>
      String(url).endsWith('/release')
        ? new Response('vendor down', { status: 500 })
        : Response.json({ id: 's-1', websocketUrl: 'wss://8.8.8.8/devtools' }),
    );
    process.env.STEEL_API_KEY = 'k';
    mock.method(console, 'error', () => {});
    stubControl({ ticket: async () => fail() });
    usage.reset();
    const persona = container.personas.resolve(KEY);
    const res = await start({ provider: 'steel' });
    assert.equal(res.statusCode, 502);
    assert.equal(container.personas.activeCount(persona.id), 0, 'slot must be freed before the vendor is asked');
    assert.equal(usage.current(KEY).openBrowsers, 0, 'the usage clock must stop for a browser that never existed');
    delete process.env.STEEL_API_KEY;
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
