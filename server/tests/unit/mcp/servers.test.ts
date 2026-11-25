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
import { CONNECTOR_TOOL_NAMES, POOL_TOOL_NAMES } from '../../../src/mcp/pool-server.ts';
import { resetSeats } from '../../../src/mcp/connector-seat.ts';
import { CARD_URI, SCREENSHOT_URI } from '../../../src/mcp/constants.ts';
import { APP_ONLY_TOOLS } from '../../../src/mcp/browser-card.ts';
import { answerFailure } from '../../../src/mcp/transport.ts';
import { registry } from '../../../src/modules/browsers/registry.ts';
import { connectBrowser, disconnectBrowser } from '../support/fakes.ts';
import { FakeResponse, driveBrowser, fakeRequest, stubControl } from '../support/browsers.ts';

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

/** A listed tool carries a non-empty title and boolean read-only and destructive hints. */
function assertAnnotated(tool: { name: string; title?: string; annotations?: Record<string, unknown> }) {
  assert.ok(tool.title, `${tool.name} has no title`);
  assert.equal(typeof tool.annotations?.readOnlyHint, 'boolean', `${tool.name} has no readOnlyHint`);
  assert.equal(typeof tool.annotations?.destructiveHint, 'boolean', `${tool.name} has no destructiveHint`);
}

/** Makes every credential a chat connector's (signed in over OAuth) on `key`. */
const connectorOn = (key: string) =>
  stubControl({
    authenticate: async () => ({ key, role: 'operator', credentialId: 'cred-chat', connector: true }),
  });

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
    connectBrowser(B, 'k-theirs', true);
    const res = await call(handleMcpRequest);
    assert.deepEqual([res.statusCode, res.body], [404, { error: `Browser ${B} not connected` }]);
  });

  it('offers every browser tool, tabs included', async () => {
    operatorOn('k-mine');
    connectBrowser(B, 'k-mine', true);
    const names = resultOf(await call(handleMcpRequest)).tools.map((t) => t.name);
    assert.ok(names.includes('list_tabs'));
    assert.ok(!names.includes('start_browser'));
  });

  it('withholds project playbooks, agent tasks and persona credential tools from share links', async () => {
    stubControl({ authenticate: async () => ({ key: 'k-mine', role: 'operator', sessionId: B }) });
    connectBrowser(B, 'k-mine', true);
    const names = resultOf(await call(handleMcpRequest)).tools.map((t) => t.name);
    assert.ok(names.includes('navigate') && names.includes('click') && names.includes('list_tabs'));
    for (const hidden of [
      'list_playbooks',
      'run_playbook',
      'run_task',
      'solve_captcha',
      'sign_in',
      'complete_mfa',
      'list_saved_logins',
    ]) {
      assert.ok(!names.includes(hidden), `${hidden} must not be exposed to a share token`);
    }
    const denied = await call(handleMcpRequest, {
      method: 'tools/call',
      params: { name: 'list_playbooks', arguments: {} },
    });
    const stream = denied.written.map((chunk) => Buffer.from(chunk).toString()).join('');
    assert.match(stream, /not found|error/i);
  });

  it('gives every tool a title and read-only and destructive hints', async () => {
    operatorOn('k-mine');
    connectBrowser(B, 'k-mine', true);
    for (const tool of resultOf(await call(handleMcpRequest)).tools) assertAnnotated(tool);
  });

  it('reads the current page as a resource', async () => {
    operatorOn('k-mine');
    connectBrowser(B, 'k-mine', true);
    registry.updateUrl(B, 'https://a.test');
    const res = await call(handleMcpRequest, { method: 'resources/read', params: { uri: 'browser://current-page' } });
    assert.equal(resultOf(res).contents[0].text, 'URL: https://a.test\nBrowser: Test');
  });

  it('answers 500 when serving the request fails', async () => {
    operatorOn('k-mine');
    connectBrowser(B, 'k-mine', true);
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
    resetSeats();
  });

  it('lists a chat connector’s tools without a key: what a chat app gets once signed in', async () => {
    const keyless = resultOf(await call(handlePoolMcpRequest, { key: '' })).tools.map((t) => t.name);
    connectorOn('k-pool');
    const signedIn = resultOf(await call(handlePoolMcpRequest)).tools.map((t) => t.name);
    assert.deepEqual(keyless, signedIn);
    assert.deepEqual([...keyless].sort(), [...CONNECTOR_TOOL_NAMES, ...APP_ONLY_TOOLS].sort());
  });

  it('keeps the whole pool, with start_browser and pool_status, for an API key', async () => {
    operatorOn('k-pool');
    const keyed = resultOf(await call(handlePoolMcpRequest)).tools.map((t) => t.name);
    assert.deepEqual([...keyed].sort(), [...POOL_TOOL_NAMES, ...APP_ONLY_TOOLS].sort());
  });

  it('gives a connector ten tools: six that act, four marked read-only, nothing to start or manage', async () => {
    connectorOn('k-pool');
    const tools = resultOf(await call(handlePoolMcpRequest)).tools.filter((t) => !APP_ONLY_TOOLS.includes(t.name));
    const reads = tools.filter((t) => t.annotations.readOnlyHint).map((t) => t.name);
    assert.equal(tools.length, 10);
    assert.deepEqual(reads.sort(), ['analyze_page', 'list_tabs', 'screenshot', 'wait_for']);
    const said = JSON.stringify(tools);
    for (const word of ['start_browser', 'pool_status', 'persona', 'provider', 'browserbase', 'this key'])
      assert.ok(!said.includes(word), `a connector's tools mention ${word}`);
    for (const tool of tools) assertAnnotated(tool);
  });

  it('tells a connector how to work, as the local MCP does, with no step to repeat each turn', async () => {
    const params = { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'chat', version: '1' } };
    const hello = resultOf(await call(handlePoolMcpRequest, { key: '', method: 'initialize', params }));
    assert.match(hello.instructions, /open_tab with a URL to start/);
    assert.match(hello.instructions, /analyze_page to read, then click\/type by element id/);
    assert.doesNotMatch(hello.instructions, /show_browser|start_browser/);
  });

  it('opens a connector’s live card beside open_tab and its capture beside screenshot', async () => {
    connectorOn('k-pool');
    const tools = resultOf(await call(handlePoolMcpRequest)).tools;
    const withCard = tools.filter((t) => t._meta?.ui?.resourceUri).map((t) => [t.name, t._meta.ui.resourceUri]);
    assert.deepEqual(withCard.sort(), [
      ['open_tab', CARD_URI],
      ['screenshot', SCREENSHOT_URI],
    ]);
  });

  it('starts a connector’s browser on its first page tool, with no start step', async () => {
    connectorOn('k-pool');
    const driver = driveBrowser(B, () => ({ ok: true }), 'k-pool', true);
    const fetch = mock.method(globalThis, 'fetch', async () => Response.json({ id: B }, { status: 201 }));
    const params = { name: 'navigate', arguments: { url: 'https://example.com' } };
    const reply = resultOf(await call(handlePoolMcpRequest, { method: 'tools/call', params }));
    assert.match(
      reply.content[0].text,
      /^Started a browser for this chat\.\n\[Driven b-mcp\] Navigated to https:\/\/example\.com$/,
    );
    assert.equal(new URL(String(fetch.mock.calls[0].arguments[0])).pathname, '/api/browsers/start');
    assert.equal(driver.sent[0].action, 'navigate');
  });

  it('answers initialize and ping without a key', async () => {
    const params = { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'scan', version: '1' } };
    assert.equal((await call(handlePoolMcpRequest, { key: '', method: 'initialize', params })).statusCode, 200);
    assert.equal((await call(handlePoolMcpRequest, { key: '', method: 'ping' })).statusCode, 200);
  });

  it('keeps tool calls behind a key', async () => {
    const params = { name: 'pool_status', arguments: {} };
    const res = await call(handlePoolMcpRequest, { key: '', method: 'tools/call', params });
    assert.deepEqual([res.statusCode, res.body], [401, { error: 'Missing or invalid API key' }]);
  });

  it('answers 401 to a wrong key, a batch or a stream sent without a key', async () => {
    assert.equal((await call(handlePoolMcpRequest, { key: 'oya_wrong' })).statusCode, 401);
    const batch = new FakeResponse();
    await handlePoolMcpRequest(fakeRequest({ key: '', headers: MCP_HEADERS, body: [{ method: 'tools/list' }] }), batch);
    assert.equal(batch.statusCode, 401);
    const stream = new FakeResponse();
    await handlePoolMcpRequest(fakeRequest({ key: '', headers: MCP_HEADERS, extra: { method: 'GET' } }), stream);
    assert.equal(stream.statusCode, 401);
  });

  it('offers the pool tools and lifecycle tools, but not tabs', async () => {
    operatorOn('k-pool');
    const names = resultOf(await call(handlePoolMcpRequest)).tools.map((t) => t.name);
    assert.ok(names.includes('start_browser') && names.includes('pool_status') && names.includes('navigate'));
    assert.ok(!names.includes('list_tabs'));
  });

  it('gives every pool tool a title and read-only and destructive hints', async () => {
    operatorOn('k-pool');
    for (const tool of resultOf(await call(handlePoolMcpRequest)).tools) assertAnnotated(tool);
  });

  it('points MCP Apps hosts at the browser card for screenshots and turn-end views, through the real SDK', async () => {
    operatorOn('k-pool');
    const tools = resultOf(await call(handlePoolMcpRequest)).tools;
    const withCard = tools.filter((t) => t._meta?.ui?.resourceUri);
    assert.deepEqual(
      withCard.map((t) => t.name),
      ['screenshot', 'show_browser'],
    );
    assert.deepEqual(
      withCard.map((t) => t._meta.ui.resourceUri),
      [SCREENSHOT_URI, CARD_URI],
    );
    const view = tools.find((t) => t.name === 'view_browser');
    assert.deepEqual(view?._meta?.ui?.visibility, ['app'], 'only the card may call view_browser');
  });

  it('serves the browser card as an MCP App page, with no credential in it', async () => {
    operatorOn('k-pool');
    const res = await call(handlePoolMcpRequest, {
      method: 'resources/read',
      params: { uri: CARD_URI },
    });
    const page = resultOf(res).contents[0];
    assert.equal(page.mimeType, 'text/html;profile=mcp-app');
    assert.doesNotMatch(page.text, /oya_op|Bearer/);
  });

  it('keeps the card and its tool off a single browser’s endpoint', async () => {
    operatorOn('k-mine');
    connectBrowser(B, 'k-mine', true);
    const tools = resultOf(await call(handleMcpRequest)).tools;
    assert.ok(!tools.some((t) => t.name === 'view_browser' || t._meta?.ui));
  });

  it('reads the pool’s status as a JSON resource', async () => {
    operatorOn('k-pool');
    connectBrowser(B, 'k-pool', true);
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
