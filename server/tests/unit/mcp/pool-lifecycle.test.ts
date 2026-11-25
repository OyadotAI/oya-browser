/**
 * Unit tests for the pool's own tools: start_browser (through the public API,
 * waiting for the browser to dial in, then holding it), stop_browser and
 * pool_status.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { registerLifecycleTools } from '../../../src/mcp/pool-lifecycle.ts';
import { hold, pickFor, poolPinned, poolSticky } from '../../../src/mcp/pool-state.ts';
import { START_POLL_MS, START_WAIT_MS } from '../../../src/mcp/constants.ts';
import { connectBrowser, disconnectBrowser } from '../support/fakes.ts';
import { FakeMcpServer, driveBrowser, stubControl } from '../support/browsers.ts';

const KEY = 'k-life';
const B = 'b-life';
const SELF = { origin: 'http://127.0.0.1:1', authorization: `Bearer ${KEY}` };

/** A server with the lifecycle tools for KEY; the public API answers with `answer(path, body)`. */
function lifecycle(answer: (path: string, body: any) => Response) {
  const fetch = mock.method(globalThis, 'fetch', async (url, init) =>
    answer(new URL(String(url)).pathname, JSON.parse(init.body)),
  );
  const server = new FakeMcpServer();
  registerLifecycleTools(server as any, { apiKey: KEY, self: SELF });
  return { server, fetch };
}

/** The text of a reply. */
const textOf = (reply) => reply.content[0].text;

/** The start endpoint's answer for B. */
const started = () => Response.json({ id: B, provider: 'oya-cloud', persona: 'p-1' }, { status: 201 });

describe('pool lifecycle tools', () => {
  beforeEach(() => stubControl());
  afterEach(() => {
    mock.restoreAll();
    mock.timers.reset();
    disconnectBrowser(B);
    disconnectBrowser('older');
    poolPinned.clear();
    poolSticky.clear();
  });

  it('starts a browser through the API as the caller and holds it', async () => {
    connectBrowser(B, KEY, true);
    const { server, fetch } = lifecycle(started);
    const reply = await server.call('start_browser', { persona: 'auto', provider: 'oya-cloud' });
    assert.match(
      textOf(reply),
      /^\[Test b-life\] ready on oya-cloud as persona p-1\. Every tool now drives this browser/,
    );
    assert.equal(new URL(fetch.mock.calls[0].arguments[0]).pathname, '/api/browsers/start');
    assert.deepEqual(JSON.parse(fetch.mock.calls[0].arguments[1].body), { profile: 'auto', provider: 'oya-cloud' });
    assert.equal(poolSticky.get(KEY), B);
  });

  it('navigates a started browser when given a URL', async () => {
    const driver = driveBrowser(B, () => ({ ok: true }), KEY, true);
    const { server } = lifecycle(started);
    assert.match(textOf(await server.call('start_browser', { url: 'https://a.test' })), /, at https:\/\/a\.test\./);
    assert.deepEqual(driver.sent[0].params, { url: 'https://a.test' });
  });

  it('reports a navigation that failed after the start', async () => {
    driveBrowser(B, () => ({ ok: false, error: 'DNS' }), KEY, true);
    const { server } = lifecycle(started);
    const reply = await server.call('start_browser', { url: 'https://a.test' });
    assert.equal(reply.isError, true);
    assert.match(textOf(reply), /started, but navigating failed: DNS/);
  });

  it('fails with the API’s error when the start is refused', async () => {
    const { server } = lifecycle(() => Response.json({ error: 'Browser quota reached (5)' }, { status: 429 }));
    assert.equal(textOf(await server.call('start_browser', {})), 'Error: Browser quota reached (5)');
  });

  it('says a browser is still booting when it does not dial in in time', async () => {
    mock.timers.enable({ apis: ['setTimeout', 'Date'] });
    const { server } = lifecycle(started);
    connectBrowser('older', KEY, true);
    hold(KEY, 'older');
    const pending = server.call('start_browser', {});
    for (let t = 0; t <= START_WAIT_MS; t += START_POLL_MS) {
      await new Promise((r) => setImmediate(r));
      mock.timers.tick(START_POLL_MS);
    }
    const reply = await pending;
    assert.match(textOf(reply), /still booting/);
    assert.equal(poolSticky.get(KEY), B);
    assert.equal(pickFor(KEY)(false), null);
    assert.match(textOf(await server.call('pool_status')), /b-life is starting/);
    connectBrowser(B, KEY, true);
    assert.equal(pickFor(KEY)(true), B);
    assert.equal(reply.structuredContent.browserId, B, 'the card can follow a browser that is still booting');
  });

  it('returns the browser identity without opening a second card at start', async () => {
    connectBrowser(B, KEY, true);
    const { server } = lifecycle(started);
    const reply = await server.call('start_browser', {});
    assert.equal(server.tools.get('start_browser').meta, undefined);
    assert.deepEqual(reply.structuredContent, { browserId: B, name: '' });
  });

  it('gives a failed start no card and no link', async () => {
    driveBrowser(B, () => ({ ok: false, error: 'DNS' }), KEY, true);
    const { server } = lifecycle(started);
    const reply = await server.call('start_browser', { url: 'https://a.test' });
    assert.equal(reply.structuredContent, undefined);
    assert.equal(reply.content.length, 1);
  });

  it('stops the browser the tools are driving and lets go of it', async () => {
    connectBrowser(B, KEY, true);
    poolPinned.set(KEY, B);
    poolSticky.set(KEY, B);
    const { server, fetch } = lifecycle(() => Response.json({ results: [{ id: B, ok: true }] }));
    assert.equal(textOf(await server.call('stop_browser', {})), `Stopped ${B}.`);
    assert.deepEqual(JSON.parse(fetch.mock.calls[0].arguments[1].body), { ids: [B] });
    assert.equal(poolPinned.has(KEY), false);
  });

  it('fails when there is no browser to stop', async () => {
    const { server } = lifecycle(() => Response.json({}));
    assert.equal(textOf(await server.call('stop_browser', {})), 'Error: no browser to stop');
  });

  it('reports a stop the API could not carry out', async () => {
    connectBrowser(B, KEY, true);
    const { server } = lifecycle(() => Response.json({ results: [{ id: B, ok: false }] }));
    assert.equal(textOf(await server.call('stop_browser', { browser_id: B })), `Error: could not stop ${B}`);
  });

  it('fails with the API’s error when the stop call itself fails', async () => {
    connectBrowser(B, KEY, true);
    const { server } = lifecycle(() => Response.json({ error: 'nope' }, { status: 500 }));
    assert.equal(textOf(await server.call('stop_browser', { browser_id: B })), 'Error: nope');
  });

  it('shows the pool with the driven browser starred', async () => {
    connectBrowser(B, KEY, true);
    poolPinned.set(KEY, B);
    const { server } = lifecycle(() => Response.json({}));
    assert.equal(textOf(await server.call('pool_status')), `Cloud browsers: 1\n  Test (${B}) ★, idle`);
  });
});
