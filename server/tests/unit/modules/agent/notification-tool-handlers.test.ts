/** Agent notification tools retain structured answers and use the regular control gate. */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';
ownDataDir();
const { executeTool } = await import('../../../../src/modules/agent/executor.ts');
const { NOTIFICATION_TOOL_NAMES } = await import('../../../../src/modules/agent/notification-tools.ts');
const { toolsOn } = await import('../../../../src/modules/agent/tools.ts');
const { scriptedBrowser, scriptedOyaBrowser } = await import('../../support/agent.ts');
const { stubControl } = await import('../../support/browsers.ts');
let browser;
afterEach(() => {
  browser?.disconnect();
  mock.restoreAll();
});
describe('agent notification tools', () => {
  it('dispatches every command without losing structured pagination or mutation results', async () => {
    stubControl();
    const data = {
      entries: [{ id: 'one', source: 'https://a.test', message: 'Reminder', time: 1, read: false }],
      total: 2,
      next_offset: 1,
    };
    browser = scriptedOyaBrowser('notification-agent', 'key-a', () => ({ ok: true, data }));
    for (const name of NOTIFICATION_TOOL_NAMES) {
      const args =
        name === 'list_notifications'
          ? { unread_only: true, limit: 1 }
          : name === 'mark_notifications_read'
            ? { ids: ['one'] }
            : { notification_id: 'one', confirm: true };
      assert.deepEqual(JSON.parse(await executeTool('notification-agent', name, args)), data);
      assert.deepEqual(browser.calls.at(-1).params, args);
      assert.equal(browser.calls.at(-1).action, name);
    }
  });
  it('returns desktop refusals as tool errors', async () => {
    stubControl();
    browser = scriptedBrowser(
      'notification-agent',
      'key-a',
      () => ({ ok: false, error: 'Confirmation required' }),
      'oya',
    );
    assert.equal(await executeTool('notification-agent', 'dismiss_notification', {}), 'Error: Confirmation required');
  });
  it('cannot bypass the control plane when a human holds the browser', async () => {
    stubControl({
      beginCommand: async () => {
        throw new Error('Human holds control');
      },
    });
    browser = scriptedOyaBrowser('notification-agent', 'key-a', () => ({ ok: true }));
    assert.equal(await executeTool('notification-agent', 'list_notifications', {}), 'Error: Human holds control');
    assert.equal(browser.calls.length, 0);
  });
  it('does not advertise these tools on browsers that lack their capabilities', () => {
    const unsupported = toolsOn(['analyze', 'list_tabs']).map((tool) => tool.function.name);
    assert.ok(NOTIFICATION_TOOL_NAMES.every((name) => !unsupported.includes(name)));
    const supported = toolsOn(NOTIFICATION_TOOL_NAMES).map((tool) => tool.function.name);
    assert.ok(NOTIFICATION_TOOL_NAMES.every((name) => supported.includes(name)));
  });
  it('refuses notification commands on generic CDP browsers rather than returning an empty notification', async () => {
    stubControl();
    browser = scriptedBrowser('notification-agent', 'key-a', () => ({ ok: true }), 'cdp');
    assert.match(await executeTool('notification-agent', 'list_notifications', {}), /not supported/);
    assert.equal(browser.calls.length, 0);
  });
});
