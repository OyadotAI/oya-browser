/** Agent discovery preserves desktop bindings and existing capability/control boundaries. */
import { it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';
ownDataDir();
const { executeTool } = await import('../../../../src/modules/agent/executor.ts');
const { toolsOn } = await import('../../../../src/modules/agent/tools.ts');
const { scriptedOyaBrowser, scriptedBrowser } = await import('../../support/agent.ts');
const { stubControl } = await import('../../support/browsers.ts');
let browser;
afterEach(() => {
  browser?.disconnect();
  mock.restoreAll();
});
it('returns the connected desktop result without substituting server platform keys', async () => {
  stubControl();
  const data = { platform: 'win32', shortcuts: [{ command: 'shortcuts', shortcut: 'Ctrl /' }] };
  browser = scriptedOyaBrowser('keyboard-agent', 'key-a', () => ({ ok: true, data }));
  assert.deepEqual(JSON.parse(await executeTool('keyboard-agent', 'list_keyboard_shortcuts', {})), data);
  assert.deepEqual(browser.calls[0], { action: 'list_keyboard_shortcuts', params: {} });
});
it('advertises discovery only when the desktop supports it', () => {
  const names = (actions) => toolsOn(actions).map((tool) => tool.function.name);
  assert.ok(names(['list_keyboard_shortcuts']).includes('list_keyboard_shortcuts'));
  assert.ok(!names(['analyze']).includes('list_keyboard_shortcuts'));
});
it('reports desktop errors without inventing bindings', async () => {
  stubControl();
  browser = scriptedOyaBrowser('keyboard-agent', 'key-a', () => ({ ok: false, error: 'Unavailable' }));
  assert.equal(await executeTool('keyboard-agent', 'list_keyboard_shortcuts', {}), 'Error: Unavailable');
});
it('does not bypass command admission', async () => {
  stubControl({
    beginCommand: async () => {
      throw new Error('Human holds control');
    },
  });
  browser = scriptedOyaBrowser('keyboard-agent', 'key-a', () => ({ ok: true }));
  assert.equal(await executeTool('keyboard-agent', 'list_keyboard_shortcuts', {}), 'Error: Human holds control');
  assert.equal(browser.calls.length, 0);
});
it('refuses discovery on generic CDP providers', async () => {
  stubControl();
  browser = scriptedBrowser('keyboard-agent', 'key-a', () => ({ ok: true }), 'cdp');
  assert.match(await executeTool('keyboard-agent', 'list_keyboard_shortcuts', {}), /not supported/);
  assert.equal(browser.calls.length, 0);
});
