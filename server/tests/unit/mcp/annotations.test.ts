/**
 * Unit tests for the MCP tool annotations: the table names exactly the tools
 * the servers register, no more and no fewer; a tool without an entry cannot be
 * registered; and the config carries the title and hints in the places clients
 * read them, with the description and schema untouched.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';
import { TOOL_META, toolConfig } from '../../../src/mcp/annotations.ts';
import { BROWSER_TOOL_NAMES } from '../../../src/mcp/browser-tools.ts';
import { POOL_TOOL_NAMES } from '../../../src/mcp/pool-server.ts';

describe('tool annotations', () => {
  it('names every registered tool and nothing else', () => {
    const registered = [...new Set([...BROWSER_TOOL_NAMES, ...POOL_TOOL_NAMES])].sort();
    assert.deepEqual(Object.keys(TOOL_META).sort(), registered);
  });

  it('never marks a read-only tool destructive', () => {
    for (const [name, meta] of Object.entries(TOOL_META)) {
      assert.ok(meta.title.length > 0, `${name} has no title`);
      assert.ok(!(meta.readOnlyHint && meta.destructiveHint), `${name} is both read-only and destructive`);
    }
  });

  it('refuses a tool with no entry, naming it', () => {
    assert.throws(() => toolConfig('nope', 'd', {}), /nope/);
  });

  it('puts the title and hints where clients read them and passes the rest through', () => {
    const schema = { url: z.string() };
    const config = toolConfig('navigate', 'Go somewhere', schema);
    assert.equal(config.title, 'Navigate');
    assert.equal(config.description, 'Go somewhere');
    assert.equal(config.inputSchema, schema);
    assert.deepEqual(config.annotations, {
      title: 'Navigate',
      readOnlyHint: false,
      destructiveHint: false,
      openWorldHint: true,
    });
  });
});
