/**
 * Unit tests for the start that needs no configuration: a start naming no
 * provider borrows the connected browser, else runs on Oya Cloud where this
 * deployment runs it, else is told the three ways on.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { cloudWhenNothingConnected } from '../../../../../src/modules/browsers/lifecycle/start-connected.ts';
import { startBrowser } from '../../../../../src/modules/browsers/lifecycle/start.ts';
import { connectBrowser, disconnectBrowser } from '../../../support/fakes.ts';
import { FakeResponse, fakeRequest, stubControl } from '../../../support/browsers.ts';
import { restoreEnv } from '../../../support/data-dir.ts';

const KEY = 'start-connected-key';
const DESKTOP = 'b-desktop';
/** What makes this deployment run Oya Cloud sandboxes. */
const CLOUD = { OYA_CLOUD_API_KEY: 'cloud-key', OYA_CLOUD_SNAPSHOT: 'oya-browser:1', OYA_PUBLIC_WS_URL: 'wss://o/ws' };
const ENV = [...Object.keys(CLOUD), 'OYA_BROWSER_PROVIDER', 'OYA_CDP_WS_URL', 'OYA_MANAGED_RUNTIME'];
let saved: Record<string, string | undefined>;

/** The provider the middleware leaves on a start with `body`. */
function providerAfter(body: object): string | undefined {
  const req = fakeRequest({ key: KEY, body });
  let ran = false;
  cloudWhenNothingConnected(req, new FakeResponse(), () => (ran = true));
  assert.ok(ran, 'the start always carries on');
  return req.body.provider;
}

describe('cloudWhenNothingConnected', () => {
  beforeEach(() => {
    saved = Object.fromEntries(ENV.map((k) => [k, process.env[k]]));
    for (const k of ENV) delete process.env[k];
    Object.assign(process.env, CLOUD);
  });
  afterEach(() => {
    for (const [k, v] of Object.entries(saved)) restoreEnv(k, v);
    disconnectBrowser(DESKTOP);
    mock.restoreAll();
  });

  it('names Oya Cloud for a start with nothing named and nothing connected', () => {
    assert.equal(providerAfter({}), 'oya-cloud');
  });

  it('leaves the connected desktop to be borrowed', () => {
    connectBrowser(DESKTOP, KEY);
    assert.equal(providerAfter({}), undefined);
  });

  it('leaves a provider the caller named', () => {
    assert.equal(providerAfter({ provider: 'cdp' }), 'cdp');
  });

  it('leaves a start that brings its own CDP URL', () => {
    assert.equal(providerAfter({ wsUrl: 'wss://8.8.8.8/devtools' }), undefined);
  });

  it('leaves a governed start out of Oya Cloud', () => {
    assert.equal(providerAfter({ governed: true }), undefined);
  });

  it('leaves the start alone where this deployment runs no sandboxes', () => {
    for (const k of Object.keys(CLOUD)) delete process.env[k];
    assert.equal(providerAfter({}), undefined);
  });

  it('still answers the three ways on when there is no cloud and nothing connected', async () => {
    for (const k of Object.keys(CLOUD)) delete process.env[k];
    stubControl();
    const res = new FakeResponse();
    const req = fakeRequest({ key: KEY, body: {}, extra: { controlSession: { id: 'b-none' } } });
    await startBrowser(req, res);
    assert.equal(res.statusCode, 409);
    assert.match(res.body.error, /open the Oya desktop browser.*oya-ai\/cli init.*wsUrl/);
  });
});
