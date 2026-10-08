/** Agent library tools retain structured answers and use the regular control gate. */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';
ownDataDir();
const { executeTool } = await import('../../../../src/modules/agent/executor.ts');
const { LIBRARY_TOOL_NAMES } = await import('../../../../src/modules/agent/library-tools.ts');
const { toolsOn } = await import('../../../../src/modules/agent/tools.ts');
const { scriptedBrowser, scriptedOyaBrowser } = await import('../../support/agent.ts');
const { stubControl } = await import('../../support/browsers.ts');
let browser;
afterEach(() => {
  browser?.disconnect();
  mock.restoreAll();
});
describe('agent library tools', () => {
  it('dispatches every command without losing structured pagination or mutation results', async () => {
    stubControl();
    const data = { entries: [{ url: 'https://a.test/', title: 'A', time: 1 }], total: 2, next_offset: 1 };
    browser = scriptedOyaBrowser('library-agent', 'key-a', () => ({ ok: true, data }));
    for (const name of LIBRARY_TOOL_NAMES) {
      const args = { query: 'a', offset: 0, limit: 1 };
      assert.deepEqual(JSON.parse(await executeTool('library-agent', name, args)), data);
      assert.deepEqual(browser.calls.at(-1).params, args);
      assert.equal(browser.calls.at(-1).action, name);
    }
  });
  it('returns desktop refusals as tool errors', async () => {
    stubControl();
    browser = scriptedBrowser('library-agent', 'key-a', () => ({ ok: false, error: 'Confirmation required' }), 'oya');
    assert.equal(await executeTool('library-agent', 'clear_history', {}), 'Error: Confirmation required');
  });
  it('cannot bypass the control plane when a human holds the browser', async () => {
    stubControl({
      beginCommand: async () => {
        throw new Error('Human holds control');
      },
    });
    browser = scriptedOyaBrowser('library-agent', 'key-a', () => ({ ok: true }));
    assert.equal(await executeTool('library-agent', 'list_bookmarks', {}), 'Error: Human holds control');
    assert.equal(browser.calls.length, 0);
  });
  it('does not advertise these tools on browsers that lack their capabilities', () => {
    const unsupported = toolsOn(['analyze', 'list_tabs']).map((tool) => tool.function.name);
    assert.ok(LIBRARY_TOOL_NAMES.every((name) => !unsupported.includes(name)));
    const supported = toolsOn(LIBRARY_TOOL_NAMES).map((tool) => tool.function.name);
    assert.ok(LIBRARY_TOOL_NAMES.every((name) => supported.includes(name)));
  });
  it('refuses library commands on generic CDP browsers rather than returning an empty library', async () => {
    stubControl();
    browser = scriptedBrowser('library-agent', 'key-a', () => ({ ok: true }), 'cdp');
    assert.match(await executeTool('library-agent', 'search_history', {}), /not supported/);
    assert.equal(browser.calls.length, 0);
  });
});
