/**
 * Unit tests for the routes that look at browsers: one browser's detail (with
 * a CDP URL for anyone but a viewer) and the live-view SSE stream.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { browserDetail, liveView } from '../../../../../src/modules/browsers/http/inspect.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { connectBrowser, disconnectBrowser } from '../../../support/fakes.ts';
import { restoreEnv } from '../../../support/data-dir.ts';
import { FakeResponse, driveBrowser, fakeRequest, stubControl } from '../../../support/browsers.ts';

const B = 'b-inspect';

/** Asks for B's detail as `key`, with extra request fields. */
async function detail(key = 'key-a', extra: object = {}) {
  const res = new FakeResponse();
  await browserDetail(fakeRequest({ key, params: { browserId: B }, headers: { host: 'h' }, extra }), res);
  return res;
}

describe('browserDetail', () => {
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('describes an Oya browser with its activity and no CDP URL', async () => {
    connectBrowser(B);
    const res = await detail();
    assert.equal(res.body.id, B);
    assert.deepEqual(res.body.activity, []);
    assert.equal(res.body.cdpUrl, undefined);
  });

  it('adds a gateway CDP URL for a CDP browser', async () => {
    stubControl();
    driveBrowser(B, () => ({ ok: true }));
    assert.equal((await detail()).body.cdpUrl, `ws://h/connect?ticket=ticket-1&browser=${B}`);
  });

  it('adds a CDP URL for an Oya browser that offers the relay', async () => {
    stubControl();
    connectBrowser(B);
    registry.get(B).cdp = true;
    assert.ok((await detail()).body.cdpUrl);
  });

  it('withholds the CDP URL from a viewer', async () => {
    stubControl();
    driveBrowser(B, () => ({ ok: true }));
    assert.equal((await detail('key-a', { principal: { role: 'viewer' } })).body.cdpUrl, undefined);
  });

  it('lists the Oya actions on a cloud sandbox this server does not hold, since its app has not said', async () => {
    // A configured cloud whose listing names B for this key, as Daytona would.
    const cloud = {
      OYA_CLOUD_API_KEY: 'cloud-key',
      OYA_CLOUD_SNAPSHOT: 'oya-browser:1',
      OYA_PUBLIC_WS_URL: 'wss://o/ws',
    };
    const saved = Object.fromEntries(Object.keys(cloud).map((name) => [name, process.env[name]]));
    Object.assign(process.env, cloud);
    try {
      const { client } = await import('../../../../../src/drivers/sandbox/client.ts');
      const { ownerTag } = await import('../../../../../src/drivers/sandbox/config.ts');
      const labels = { 'oya-browser': 'true', 'oya-owner': ownerTag('key-cloud'), 'oya-browser-id': B };
      mock.method(await client(), 'list', async function* () {
        yield { state: 'started', labels };
      });
      const res = await detail('key-cloud');
      assert.equal(res.statusCode, 200);
      assert.ok(res.body.actions.includes('workflow') && !res.body.actions.includes('cookies'));
    } finally {
      for (const [name, value] of Object.entries(saved)) restoreEnv(name, value);
    }
  });

  it('answers 404 for another key’s browser, as for a missing one', async () => {
    connectBrowser(B, 'key-a');
    const res = await detail('key-b');
    assert.deepEqual([res.statusCode, res.body], [404, { error: `Browser ${B} not connected` }]);
  });
});

describe('liveView', () => {
  afterEach(() => disconnectBrowser(B));

  it('opens an SSE stream, sends the latest frame at once, and detaches on close', () => {
    connectBrowser(B);
    registry.pushFrame(B, 'frame-1');
    const req = fakeRequest({ params: { browserId: B } });
    const res = new FakeResponse();
    liveView(req, res);
    assert.equal(res.statusCode, 200);
    assert.equal(res.headers['Content-Type'], 'text/event-stream');
    assert.deepEqual(res.written, ['data: frame-1\n\n']);
    assert.equal(registry.hasViewers(B), true);
    req.emit('close');
    assert.equal(registry.hasViewers(B), false);
  });

  it('waits for the first frame when there is none yet', () => {
    connectBrowser(B);
    const res = new FakeResponse();
    liveView(fakeRequest({ params: { browserId: B } }), res);
    assert.deepEqual(res.written, []);
  });
});
