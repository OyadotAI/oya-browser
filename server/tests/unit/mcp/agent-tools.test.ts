/**
 * Unit tests for the MCP tools that run through the agent's own tool code: they
 * carry the agent's words and schemas, run its handlers on the picked browser,
 * and are left out where the browser cannot run them.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { registerBrowserTools } from '../../../src/mcp/browser-tools.ts';
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
    assert.ok(!server.tools.has('read_console') && !server.tools.has('read_network'));
    assert.ok(['go_back', 'hover', 'solve_captcha', 'run_playbook', 'run_task'].every((n) => server.tools.has(n)));
  });

  it('says a playbook that does not exist is not there', async () => {
    const { server } = tools(() => ({ ok: true }));
    const reply = await server.call('run_playbook', { name: 'nope' });
    assert.match(reply.content[0].text, /no playbook named "nope"/);
  });
});
