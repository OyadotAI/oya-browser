/**
 * Unit tests for the MCP tools that run through the agent's own tool code: they
 * carry the agent's words and schemas, run its handlers on the picked browser,
 * and are left out where the browser cannot run them.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { registerBrowserTools } from '../../../src/mcp/browser-tools.ts';
import { scriptedOyaBrowser } from '../support/agent.ts';
import { NOTIFICATION_TOOL_NAMES } from '../../../src/modules/agent/notification-tools.ts';
import { LIBRARY_TOOL_NAMES } from '../../../src/modules/agent/library-tools.ts';
import { BROWSER_TOOLS } from '../../../src/modules/agent/tools.ts';
import { disconnectBrowser } from '../support/fakes.ts';
import { FakeMcpServer, driveBrowser, stubControl } from '../support/browsers.ts';

const B = 'b-agent-tools';

/** A server with every tool aimed at B, whose driver answers with `answer`. */
function tools(answer: (action: string, params: any) => any) {
  const driver = driveBrowser(B, answer);
  const server = new FakeMcpServer();
  registerBrowserTools(server as any, { pick: () => B, oneBrowser: true });
  return { server, driver };
}

describe('agent tools over MCP', () => {
  beforeEach(() => stubControl());
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('exposes the same local library tools on the per-browser MCP server', async () => {
    const data = { entries: [], total: 0, next_offset: null };
    const browser = scriptedOyaBrowser(B, 'key-a', () => ({ ok: true, data }));
    const server = new FakeMcpServer();
    registerBrowserTools(server as any, { pick: () => B, oneBrowser: true });
    assert.ok(LIBRARY_TOOL_NAMES.every((name) => server.tools.has(name)));
    const reply = await server.call('search_history', { query: 'invoice', limit: 1 });
    assert.deepEqual(JSON.parse(reply.content[0].text), data);
    assert.equal(browser.calls[0].action, 'search_history');
    assert.equal(
      server.tools.get('search_history').description,
      BROWSER_TOOLS.find((tool) => tool.function.name === 'search_history').function.description,
    );
  });

  it('serves notification tools with the same schemas and handlers as the agent loop', async () => {
    const data = { entries: [], unread_count: 0, total: 0, next_offset: null, session_only: true };
    const browser = scriptedOyaBrowser(B, 'key-a', () => ({ ok: true, data }));
    const server = new FakeMcpServer();
    registerBrowserTools(server as any, { pick: () => B, oneBrowser: true });
    for (const name of NOTIFICATION_TOOL_NAMES) {
      assert.equal(
        server.tools.get(name).description,
        BROWSER_TOOLS.find((tool) => tool.function.name === name).function.description,
      );
      const args =
        name === 'list_notifications'
          ? { unread_only: true }
          : name === 'mark_notifications_read'
            ? { ids: ['one'] }
            : { notification_id: 'one', confirm: true };
      const reply = await server.call(name, args);
      assert.deepEqual(JSON.parse(reply.content[0].text), data);
      assert.deepEqual(browser.calls.at(-1), { action: name, params: args });
    }
  });

  it('serves keyboard discovery using the same definition as the agent loop', async () => {
    const data = { platform: 'darwin', shortcuts: [] };
    const browser = scriptedOyaBrowser(B, 'key-a', () => ({ ok: true, data }));
    const server = new FakeMcpServer();
    registerBrowserTools(server as any, { pick: () => B, oneBrowser: true });
    const reply = await server.call('list_keyboard_shortcuts', {});
    assert.deepEqual(JSON.parse(reply.content[0].text), data);
    assert.equal(browser.calls[0].action, 'list_keyboard_shortcuts');
    assert.equal(
      server.tools.get('list_keyboard_shortcuts').description,
      BROWSER_TOOLS.find((tool) => tool.function.name === 'list_keyboard_shortcuts').function.description,
    );
  });

  it('describes each tool in the words the agent is offered', () => {
    const { server } = tools(() => ({ ok: true }));
    const agent = BROWSER_TOOLS.find((t) => t.function.name === 'run_script')!.function;
    assert.equal(server.tools.get('run_script').description, agent.description);
  });

  it('runs a tool through the agent’s handler and answers its text', async () => {
    const { server, driver } = tools(() => ({ ok: true, data: { value: ['a', 'b'] } }));
    const reply = await server.call('run_script', { script: 'return ["a","b"]' });
    assert.deepEqual(JSON.parse(reply.content[0].text), ['a', 'b']);
    assert.equal(driver.sent[0].action, 'run_script');
  });

  it('answers an Error text as a failed call', async () => {
    const { server } = tools(() => ({ ok: true }));
    const reply = await server.call('run_script', { script: 'document.body.click()' });
    assert.equal(reply.isError, true);
    assert.match(reply.content[0].text, /^Error: run_script only reads the page/);
  });

  it('leaves out the tools this browser cannot run', () => {
    // A CDP browser keeps no console or network log.
    const { server } = tools(() => ({ ok: true }));
    assert.ok(LIBRARY_TOOL_NAMES.every((name) => !server.tools.has(name)));
    assert.ok(NOTIFICATION_TOOL_NAMES.every((name) => !server.tools.has(name)));
    assert.ok(!server.tools.has('read_console') && !server.tools.has('read_network'));
    assert.ok(['go_back', 'hover', 'solve_captcha', 'run_playbook', 'run_task'].every((n) => server.tools.has(n)));
  });

  it('says a playbook that does not exist is not there', async () => {
    const { server } = tools(() => ({ ok: true }));
    const reply = await server.call('run_playbook', { name: 'nope' });
    assert.match(reply.content[0].text, /no playbook named "nope"/);
  });
});
