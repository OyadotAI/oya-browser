/**
 * Unit tests for the tools the local MCP port offers a test: each becomes one browser command,
 * carries only the arguments it names, and never more time or entries than the port allows.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { LOCAL_TOOLS, toolList } from '../../../../src/main/local-mcp/tools.ts';
import { WAIT_FOR_MAX_MS } from '../../../../src/main/local-mcp/constants.ts';

it('offers the tools a UI test needs beside the page tools', () => {
  const names = toolList().map((t) => t.name);
  for (const name of ['read_console', 'read_network', 'wait_for']) assert.ok(names.includes(name), name);
});

it('every tool refuses arguments it does not name', () => {
  for (const tool of toolList()) assert.equal((tool.inputSchema as any).additionalProperties, false, tool.name);
});

it('read_console runs the console read with only the filters given', () => {
  assert.deepEqual(LOCAL_TOOLS.read_console.command({}), ['read_console', {}]);
  assert.deepEqual(LOCAL_TOOLS.read_console.command({ level: 'error', pattern: 'boom', limit: 5 }), [
    'read_console',
    { level: 'error', pattern: 'boom', limit: 5 },
  ]);
});

it('read_network asks for failures by the name the browser command uses', () => {
  assert.deepEqual(LOCAL_TOOLS.read_network.command({ failed_only: true }), ['read_network', { failedOnly: true }]);
  assert.deepEqual(LOCAL_TOOLS.read_network.command({ failed_only: 'yes' }), ['read_network', {}]);
});

it('a read never forwards a page id, a tab id or any argument it does not name', () => {
  const hostile = { webContentsId: 1, tab_id: 9, failedOnly: true, level: 'error' };
  assert.deepEqual(LOCAL_TOOLS.read_console.command(hostile)[1], { level: 'error' });
  assert.deepEqual(LOCAL_TOOLS.read_network.command(hostile)[1], {});
});

it('a limit that is not a positive number is left out, so the default applies', () => {
  for (const limit of ['all', 0, -1, Number.POSITIVE_INFINITY, null])
    assert.deepEqual(LOCAL_TOOLS.read_console.command({ limit })[1], {}, String(limit));
  assert.deepEqual(LOCAL_TOOLS.read_console.command({ limit: '5' })[1], { limit: 5 });
});

it('wait_for waits for text or a selector through the wait command', () => {
  assert.deepEqual(LOCAL_TOOLS.wait_for.command({ text: 'Saved' }), ['wait', { text: 'Saved' }]);
  assert.deepEqual(LOCAL_TOOLS.wait_for.command({ selector: '#done', timeout: 500 }), [
    'wait',
    { selector: '#done', timeout: 500 },
  ]);
});

it('wait_for never waits longer than the port allows', () => {
  assert.equal(LOCAL_TOOLS.wait_for.command({ text: 'x', timeout: WAIT_FOR_MAX_MS * 10 })[1].timeout, WAIT_FOR_MAX_MS);
  assert.equal(LOCAL_TOOLS.wait_for.command({ text: 'x', timeout: 'forever' })[1].timeout, undefined);
});
