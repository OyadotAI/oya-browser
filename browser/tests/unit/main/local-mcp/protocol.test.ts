/** The MCP conversation: what an agent is told on connecting, which tools it gets, and how a tool call reads back. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { answer, toolReply } from '../../../../src/main/local-mcp/protocol.ts';

/** A runner that records commands and answers `result`. */
function runner(result = { ok: true, data: {}, error: null as string | null }) {
  const ran: unknown[][] = [];
  return { ran, run: async (action: string, params: object) => (ran.push([action, params]), result) };
}

it('introduces itself on initialize, with tools and how to behave', async () => {
  const reply: any = await answer({ jsonrpc: '2.0', id: 1, method: 'initialize', params: {} }, runner().run);
  assert.equal(reply.result.serverInfo.name, 'Oya Browser');
  assert.ok(reply.result.capabilities.tools);
  assert.match(reply.result.instructions, /own tab/);
  assert.match(reply.result.instructions, /in place of other browser tools/);
  assert.match(reply.result.instructions, /testing an app on localhost and extracting data from pages/);
});

it('lists the same tools the hosted MCP offers', async () => {
  const reply: any = await answer({ jsonrpc: '2.0', id: 2, method: 'tools/list' }, runner().run);
  const names = reply.result.tools.map((t: any) => t.name);
  for (const name of ['analyze_page', 'navigate', 'click', 'type', 'open_tab', 'switch_tab', 'close_tab', 'screenshot'])
    assert.ok(names.includes(name), name);
});

it('runs a tool as its browser command and reads the page back as text', async () => {
  const r = runner({ ok: true, data: { page: 'blocks[1]...' }, error: null });
  const reply: any = await answer(
    { jsonrpc: '2.0', id: 3, method: 'tools/call', params: { name: 'click', arguments: { element_id: 7 } } },
    r.run,
  );
  assert.deepEqual(r.ran, [['click', { selector: '[data-ac-id="7"]' }]]);
  assert.deepEqual(reply.result.content, [{ type: 'text', text: 'blocks[1]...' }]);
});

it('returns a screenshot as an image', () => {
  const reply = toolReply({ ok: true, data: { screenshot: 'data:image/png;base64,AAAA' }, error: null });
  assert.deepEqual(reply.content, [{ type: 'image', data: 'AAAA', mimeType: 'image/png' }]);
});

it('reports a refused command as a tool error with its reason', () => {
  const reply = toolReply({
    ok: false,
    data: null,
    error: 'Not closed: an agent may close only tabs it opened itself',
  });
  assert.equal(reply.isError, true);
  assert.match(reply.content[0].text, /only tabs it opened/);
});

it('refuses an unknown tool without running anything', async () => {
  const r = runner();
  const reply: any = await answer(
    { jsonrpc: '2.0', id: 4, method: 'tools/call', params: { name: 'evaluate_raw' } },
    r.run,
  );
  assert.equal(reply.result.isError, true);
  assert.deepEqual(r.ran, []);
});

it('gives a notification no reply, and a malformed or unknown request an error', async () => {
  assert.equal(await answer({ jsonrpc: '2.0', method: 'notifications/initialized' }, runner().run), null);
  assert.equal(((await answer({ hello: 1 }, runner().run)) as any).error.code, -32_600);
  assert.equal(
    ((await answer({ jsonrpc: '2.0', id: 5, method: 'resources/list' }, runner().run)) as any).error.code,
    -32_601,
  );
});
