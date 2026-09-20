/**
 * Unit tests for the MCP endpoints' handlers (browser-server.ts,
 * pool-server.ts and transport.ts, through the server.ts entry point), driven
 * with JSON-RPC requests over a fake Express request: who may reach an
 * endpoint, what each server offers, and a request that fails before answering.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { handleMcpRequest, handlePoolMcpRequest } from '../../../src/mcp/server.ts';
import { answerFailure } from '../../../src/mcp/transport.ts';
import { registry } from '../../../src/modules/browsers/registry.ts';
import { connectBrowser, disconnectBrowser } from '../support/fakes.ts';
import { FakeResponse, fakeRequest, stubControl } from '../support/browsers.ts';

const B = 'b-mcp';

/** Headers a Streamable HTTP client sends with a JSON-RPC POST. */
const MCP_HEADERS = { host: 'h', accept: 'application/json, text/event-stream', 'content-type': 'application/json' };

/** A JSON-RPC POST to `handler` for B as `key`; returns the response once it has ended. */
async function call(handler, { key = 'oya_op', method = 'tools/list', params = {} } = {}) {
  const res = new FakeResponse();
  const req = fakeRequest({
    key,
    params: { browserId: B },
    headers: MCP_HEADERS,
    body: { jsonrpc: '2.0', id: 1, method, params },
    extra: { url: `/mcp/${B}`, rawHeaders: Object.entries(MCP_HEADERS).flat(), socket: { localPort: 1 } },
  });
  await handler(req, res);
  for (let i = 0; i < 100 && res.ended === null && !res.body; i++) await new Promise((r) => setImmediate(r));
  return res;
}

/** The JSON-RPC result a streamed MCP answer carries. */
function resultOf(res: FakeResponse) {
  const stream = res.written.map((chunk) => Buffer.from(chunk).toString()).join('');
  return JSON.parse(stream.split('data: ')[1]).result;
}

/** Makes every credential an operator on `key`. */
const operatorOn = (key: string) => stubControl({ authenticate: async () => ({ key, role: 'operator' }) });

describe('per-browser MCP endpoint', () => {
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('answers 401 without a key', async () => {
    assert.equal((await call(handleMcpRequest, { key: '' })).statusCode, 401);
  });

  it('answers 404 for a browser that belongs to another key, as for a missing one', async () => {
    operatorOn('k-mine');
    connectBrowser(B, 'k-theirs');
    const res = await call(handleMcpRequest);
    assert.deepEqual([res.statusCode, res.body], [404, { error: `Browser ${B} not connected` }]);
  });

  it('offers every browser tool, tabs included', async () => {
    operatorOn('k-mine');
    connectBrowser(B, 'k-mine');
    const names = resultOf(await call(handleMcpRequest)).tools.map((t) => t.name);
    assert.ok(names.includes('list_tabs'));
    assert.ok(!names.includes('start_browser'));
  });

  it('reads the current page as a resource', async () => {
    operatorOn('k-mine');
    connectBrowser(B, 'k-mine');
    registry.updateUrl(B, 'https://a.test');
    const res = await call(handleMcpRequest, { method: 'resources/read', params: { uri: 'browser://current-page' } });
    assert.equal(resultOf(res).contents[0].text, 'URL: https://a.test\nBrowser: Test');
  });

  it('answers 500 when serving the request fails', async () => {
    operatorOn('k-mine');
    connectBrowser(B, 'k-mine');
    mock.method(McpServer.prototype, 'connect', async () => {
      throw new Error('transport broke');
    });
    mock.method(console, 'error', () => {});
    const res = await call(handleMcpRequest);
    assert.deepEqual([res.statusCode, res.body], [500, { error: 'transport broke' }]);
  });
});

describe('pool MCP endpoint', () => {
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('answers 401 without a key', async () => {
    assert.equal((await call(handlePoolMcpRequest, { key: '' })).statusCode, 401);
  });

  it('offers the pool tools and lifecycle tools, but not tabs', async () => {
    operatorOn('k-pool');
    const names = resultOf(await call(handlePoolMcpRequest)).tools.map((t) => t.name);
    assert.ok(names.includes('start_browser') && names.includes('pool_status') && names.includes('navigate'));
    assert.ok(!names.includes('list_tabs'));
  });

  it('reads the pool’s status as a JSON resource', async () => {
    operatorOn('k-pool');
    connectBrowser(B, 'k-pool');
    const res = await call(handlePoolMcpRequest, {
      method: 'resources/read',
      params: { uri: 'browser://pool-status' },
    });
    assert.equal(JSON.parse(resultOf(res).contents[0].text).size, 1);
  });

  it('answers 500 when serving the request fails', async () => {
    operatorOn('k-pool');
    mock.method(McpServer.prototype, 'connect', async () => {
      throw new Error('transport broke');
    });
    mock.method(console, 'error', () => {});
    assert.equal((await call(handlePoolMcpRequest)).statusCode, 500);
  });
});

describe('answerFailure', () => {
  it('answers 500 with the error, unless a response is already under way', () => {
    const fresh = new FakeResponse();
    answerFailure(fresh, new Error('boom'));
    assert.deepEqual([fresh.statusCode, fresh.body], [500, { error: 'boom' }]);
    const started = new FakeResponse();
    started.headersSent = true;
    answerFailure(started, new Error('late'));
    assert.equal(started.body, undefined);
  });
});
