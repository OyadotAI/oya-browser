/**
 * Unit tests for the browser card MCP Apps hosts show for start_browser: its page is an
 * MCP App with no credential in it, only the card may call view_browser, and view_browser
 * answers only for the caller's own connected browser, says when a person holds control,
 * and never puts its snapshots in the live page's activity log.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { registerBrowserCard, viewBrowser, CARD_META } from '../../../src/mcp/browser-card.ts';
import { awaitBrowser, letGo } from '../../../src/mcp/pool-state.ts';
import { CARD_MIME, CARD_URI } from '../../../src/mcp/constants.ts';
import { registry } from '../../../src/modules/browsers/registry.ts';
import { disconnectBrowser } from '../support/fakes.ts';
import { FakeMcpServer, driveBrowser, stubControl } from '../support/browsers.ts';
import { HttpError } from '../../../src/platform/errors.ts';

const KEY = 'k-card';
const B = 'b-card';
const SHOT = 'data:image/jpeg;base64,AAAA';

/** B, connected on KEY, answering screenshots with `answer`. */
const browser = (answer = () => ({ ok: true, data: { screenshot: SHOT } })) => driveBrowser(B, answer, KEY, true);

describe('browser card', () => {
  beforeEach(() => stubControl());
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
    letGo(KEY, B);
  });

  it('serves its page as an MCP App with no credential in it', async () => {
    const server = new FakeMcpServer();
    registerBrowserCard(server as any, KEY);
    const page = (await server.resources.get('browser-card').read()).contents[0];
    assert.equal(server.resources.get('browser-card').uri, CARD_URI);
    assert.equal(page.mimeType, CARD_MIME);
    assert.match(page.text, /ui\/initialize/);
    assert.doesNotMatch(page.text, /Bearer|api[_-]?key|oya_[a-z0-9]{8}/i);
    assert.deepEqual(CARD_META, { ui: { resourceUri: CARD_URI } });
  });

  it('lets only the card call view_browser, never the model', () => {
    const server = new FakeMcpServer();
    registerBrowserCard(server as any, KEY);
    assert.deepEqual(server.tools.get('view_browser').meta, { ui: { visibility: ['app'] } });
    assert.equal(server.tools.get('view_browser').annotations.readOnlyHint, true);
  });

  it('presents an owned browser on successive calls and refuses missing or foreign browsers', async () => {
    browser();
    const server = new FakeMcpServer();
    registerBrowserCard(server as any, KEY);
    for (const turn of [1, 2]) {
      const reply = await server.call('show_browser', { browser_id: B });
      assert.equal(reply.structuredContent.browserId, B, `turn ${turn}`);
    }
    assert.equal((await server.call('show_browser', { browser_id: 'missing' })).isError, true);
    registerBrowserCard(server as any, 'other');
    assert.equal((await server.call('show_browser', { browser_id: B })).isError, true);
  });

  it('presents the reserved worker while waiting for its authenticated connection', async () => {
    awaitBrowser(KEY, B);
    const server = new FakeMcpServer();
    registerBrowserCard(server as any, KEY);
    const reply = await server.call('show_browser', { browser_id: B });
    assert.equal(reply.structuredContent.pending, true);
    assert.deepEqual((await viewBrowser(KEY, B)).structuredContent, { state: 'unavailable' });
    registerBrowserCard(server as any, 'other');
    assert.equal((await server.call('show_browser', { browser_id: B })).isError, true);
  });

  it('returns the watch credential only in component metadata', async () => {
    const server = new FakeMcpServer();
    const fetch = mock.method(globalThis, 'fetch', async () => Response.json({ token: 'temporary' }));
    registerBrowserCard(server as any, KEY, { origin: 'http://localhost', authorization: 'Bearer caller' });
    const reply = await server.call('watch_browser', { browser_id: B });
    assert.equal(fetch.mock.calls[0].arguments[1].headers.Authorization, 'Bearer caller');
    assert.equal(server.tools.get('watch_browser').meta.ui.visibility[0], 'app');
    assert.deepEqual(reply.content, []);
    assert.equal(reply.structuredContent, undefined);
    assert.match(reply._meta.watchUrl, /#t=temporary$/);
  });

  it('shows the caller’s own browser live, with its picture', async () => {
    browser();
    const reply = await viewBrowser(KEY, B);
    assert.equal(reply.structuredContent.state, 'live');
    assert.equal(reply.structuredContent.image, SHOT);
  });

  it('takes a bare screenshot payload to be PNG', async () => {
    browser(() => ({ ok: true, data: { screenshot: 'AAAA' } }));
    assert.equal((await viewBrowser(KEY, B)).structuredContent.image, 'data:image/png;base64,AAAA');
  });

  it('answers another key’s browser as unavailable and sends it nothing', async () => {
    const driver = browser();
    assert.deepEqual((await viewBrowser('someone-else', B)).structuredContent, { state: 'unavailable' });
    assert.deepEqual(driver.sent, []);
  });

  it('answers a browser that is not connected as unavailable, the same as another key’s', async () => {
    assert.deepEqual((await viewBrowser(KEY, 'b-gone')).structuredContent, { state: 'unavailable' });
  });

  it('says a person holds control when the control fence refuses the snapshot', async () => {
    browser();
    stubControl({
      beginCommand: async () => {
        throw new HttpError(409, 'paused', { code: 'control_paused' });
      },
    });
    assert.equal((await viewBrowser(KEY, B)).structuredContent.state, 'held');
  });

  it('reports any other failure as failed, so the card can say it is retrying', async () => {
    browser(() => ({ ok: false, error: 'capture failed' }));
    assert.equal((await viewBrowser(KEY, B)).structuredContent.state, 'failed');
    browser(() => ({ ok: true, data: {} }));
    assert.equal((await viewBrowser(KEY, B)).structuredContent.state, 'failed');
    browser(() => {
      throw new Error('boom');
    });
    assert.equal((await viewBrowser(KEY, B)).structuredContent.state, 'failed');
  });

  it('keeps its snapshots out of the live page’s activity log, and the browser never sees the flag', async () => {
    const driver = browser();
    const pending = registry.get(B).pending;
    await viewBrowser(KEY, B);
    assert.deepEqual(JSON.parse(JSON.stringify(driver.sent[0].params)), { format: 'jpeg', quality: 55 });
    assert.equal(registry.get(B).pending, pending, 'a hidden command is never counted as the browser doing something');
  });
});
