/**
 * Unit tests for the browser tools shared by the MCP servers: each tool sends
 * one command to the picked browser and turns the result into a reply.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { registerBrowserTools } from '../../../src/mcp/browser-tools.ts';
import { NAVIGATE_TIMEOUT_MS, SCROLL_TIMEOUT_MS } from '../../../src/mcp/constants.ts';
import { disconnectBrowser } from '../support/fakes.ts';
import { FakeMcpServer, driveBrowser, stubControl } from '../support/browsers.ts';

const B = 'b-tools';

/** A server with every tool aimed at B, whose driver answers with `answer`. */
function tools(answer: (action: string, params: any) => any, options: object = {}) {
  const driver = driveBrowser(B, answer);
  const server = new FakeMcpServer();
  registerBrowserTools(server as any, { pick: () => B, ...options });
  return { server, driver };
}

/** The text of a reply. */
const textOf = (reply) => reply.content[0].text;

describe('browser tools', () => {
  beforeEach(() => stubControl());
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('registers every tool unless told to register only some', () => {
    const { server } = tools(() => ({ ok: true }));
    assert.ok(server.tools.has('list_tabs'));
    const limited = new FakeMcpServer();
    registerBrowserTools(limited as any, { pick: () => B, only: ['click'] });
    assert.deepEqual([...limited.tools.keys()], ['click']);
  });

  it('clicks an element by the number analyze gave it', async () => {
    const { server, driver } = tools(() => ({ ok: true }));
    assert.equal(textOf(await server.call('click', { element_id: 4 })), 'Clicked element 4');
    assert.deepEqual(driver.sent[0].params, { selector: '[data-ac-id="4"]' });
  });

  it('navigates with the long navigation timeout', async () => {
    const { server, driver } = tools(() => ({ ok: true }));
    assert.equal(textOf(await server.call('navigate', { url: 'https://a.test' })), 'Navigated to https://a.test');
    assert.equal(driver.sent[0].timeout, NAVIGATE_TIMEOUT_MS);
  });

  it('returns an analysis as the page text', async () => {
    const { server } = tools(() => ({ ok: true, data: { markdown: '# Hi', elements: [] } }));
    assert.match(textOf(await server.call('analyze_page')), /^# Hi\n\n## Element Index \(0 total/);
  });

  it('returns a screenshot as a PNG image without its data-URL prefix', async () => {
    const { server } = tools(() => ({ ok: true, data: { screenshot: 'data:image/png;base64,QUJD' } }));
    assert.deepEqual(await server.call('screenshot'), {
      content: [{ type: 'image', data: 'QUJD', mimeType: 'image/png' }],
    });
  });

  it('says so when a screenshot comes back empty', async () => {
    const { server } = tools(() => ({ ok: true, data: {} }));
    assert.equal(textOf(await server.call('screenshot')), 'Screenshot captured but no image data returned');
  });

  it('reports a plain scroll, or the page it re-analyzed', async () => {
    const { server, driver } = tools(() => ({ ok: true, data: {} }));
    assert.equal(textOf(await server.call('scroll', { direction: 'down' })), 'Scrolled down 500px');
    assert.equal(driver.sent[0].timeout, SCROLL_TIMEOUT_MS);
    disconnectBrowser(B);
    const again = tools(() => ({ ok: true, data: { markdown: 'M', elements: [] } }));
    assert.match(textOf(await again.server.call('scroll', { direction: 'up', amount: 10 })), /^M\n\n## Element Index/);
  });

  it('double-clicks an element, or a point when no element is given', async () => {
    const { server, driver } = tools(() => ({ ok: true }));
    assert.equal(textOf(await server.call('double_click', { element_id: 2 })), 'Double-clicked element 2');
    assert.equal(textOf(await server.call('double_click', { x: 1, y: 2 })), 'Double-clicked at (1, 2)');
    assert.deepEqual(driver.sent[1].params, { x: 1, y: 2 });
  });

  it('lists elements with their best label', async () => {
    const data = {
      url: 'u',
      title: 'T',
      elements: [{ tag: 'button', id: 'go', text: 'Go' }, { tag: 'input', placeholder: 'Email' }, { tag: 'div' }],
    };
    const { server, driver } = tools(() => ({ ok: true, data }));
    const out = textOf(await server.call('read_elements', { limit: 3 }));
    assert.equal(out, 'Page: T (u)\n\nElements (3):\nbutton#go — Go\ninput — Email\ndiv — (no text)');
    assert.equal(driver.sent[0].action, 'read_page');
  });

  it('lists tabs with the active one marked', async () => {
    const data = {
      tabs: [
        { id: 1, title: 'A', url: 'a', active: true },
        { id: 2, title: 'B', url: 'b' },
      ],
    };
    const { server } = tools(() => ({ ok: true, data }));
    assert.equal(textOf(await server.call('list_tabs')), 'Tabs:\n→ [tab 1] A — a\n  [tab 2] B — b');
  });

  it('describes the rest of the tools’ results in a line', async () => {
    const { server } = tools((action) => ({
      ok: true,
      data: action === 'open_tab' ? { tab_id: 9 } : { accepted: false, type: 'confirm' },
    }));
    const lines = [
      [await server.call('type', { element_id: 1, text: 'hi' }), 'Typed "hi" into element 1'],
      [await server.call('press_key', { key: 'Enter' }), 'Pressed Enter'],
      [await server.call('wait', { selector: '#x' }), 'Element found: #x'],
      [await server.call('click_coordinates', { x: 1, y: 2 }), 'Clicked at (1, 2)'],
      [await server.call('mouse_move', { x: 3, y: 4 }), 'Mouse moved to (3, 4)'],
      [await server.call('keyboard_type', { text: 'yo' }), 'Typed "yo"'],
      [await server.call('drag', { from_x: 1, from_y: 2, to_x: 3, to_y: 4 }), 'Dragged from (1, 2) to (3, 4)'],
      [await server.call('open_tab', { url: 'https://a.test' }), 'Opened tab 9 at https://a.test'],
      [await server.call('switch_tab', { tab_id: 2 }), 'Switched to tab 2'],
      [await server.call('handle_dialog', { accept: false }), 'Dismissed the confirm dialog'],
      [await server.call('close_tab', {}), 'Closed tab'],
    ];
    for (const [reply, expected] of lines) assert.equal(textOf(reply), expected);
  });

  it('prefixes text replies with the answering browser’s tag', async () => {
    const { server } = tools(() => ({ ok: true, data: { markdown: 'M', elements: [] } }), { tag: () => '[B]' });
    assert.equal(textOf(await server.call('click', { element_id: 1 })), '[B] Clicked element 1');
    assert.match(textOf(await server.call('analyze_page')), /^\[B\]\nM/);
  });

  it('fails the call when the browser reports an error', async () => {
    const { server } = tools(() => ({ ok: false, error: 'no such element' }));
    assert.deepEqual(await server.call('click', { element_id: 1 }), {
      content: [{ type: 'text', text: 'Error: no such element' }],
      isError: true,
    });
  });

  it('fails the call when there is no browser to pick', async () => {
    const server = new FakeMcpServer();
    registerBrowserTools(server as any, { pick: () => null, noBrowser: 'start one first' });
    assert.equal(textOf(await server.call('click', { element_id: 1 })), 'Error: start one first');
  });
});
