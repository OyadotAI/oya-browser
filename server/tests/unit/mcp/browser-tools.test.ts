/**
 * Unit tests for the browser tools shared by the MCP servers: each tool sends
 * one command to the picked browser and turns the result into a reply.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { registerBrowserTools } from '../../../src/mcp/browser-tools.ts';
import { canonical, supportedOn } from '../../../src/drivers/vocabulary.ts';
import { CARD_URI, NAVIGATE_TIMEOUT_MS, SCROLL_TIMEOUT_MS } from '../../../src/mcp/constants.ts';
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

  it('sends only actions every kind of browser does, so no tool is refused on a pool or a desktop browser', async () => {
    const { server, driver } = tools(() => ({ ok: true, data: {} }));
    const args = {
      element_id: 1,
      url: 'https://a.test',
      selector: '#a',
      tab_id: 1,
      key: 'Enter',
      text: 'x',
      x: 1,
      y: 2,
    };
    for (const name of server.tools.keys()) await server.call(name, args).catch(() => {});
    const sent = new Set(driver.sent.map((c) => c.action));
    for (const action of sent) {
      assert.deepEqual(supportedOn(canonical(action)!), ['oya', 'cdp'], `${action} is not on both kinds`);
    }
    assert.ok(sent.size > 5, `only ${sent.size} actions reached the browser: the tools did not run`);
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

  it("asks the browser for the page in the format the caller picks, else the server's TOON", async () => {
    const { server, driver } = tools(() => ({ ok: true, data: { markdown: '# Hi', elements: [] } }));
    await server.call('analyze_page', { format: 'markdown' });
    await server.call('analyze_page');
    assert.deepEqual(
      driver.sent.map((s) => s.params.format),
      ['markdown', 'toon'],
    );
  });

  it('offers only the page formats there is a renderer for', () => {
    const { server } = tools(() => ({ ok: true, data: {} }));
    const format = (server.tools.get('analyze_page').schema as any).format;
    for (const name of ['markdown', 'toon', 'jsonl']) assert.ok(format.safeParse(name).success, name);
    assert.equal(format.safeParse('xml').success, false);
  });

  it('gives a chat widget the same immutable capture the client receives', async () => {
    const { server } = tools(() => ({ ok: true, data: { screenshot: 'data:image/png;base64,QUJD' } }), {
      screenshotCard: true,
    });
    const reply = await server.call('screenshot', { caption: '1. Search results' });
    assert.deepEqual(reply._meta, { capture: reply.content[0], caption: '1. Search results' });
    assert.equal(reply.content[0].data, 'QUJD');
  });

  it('returns a screenshot as a PNG image without its data-URL prefix', async () => {
    const { server } = tools(() => ({ ok: true, data: { screenshot: 'data:image/png;base64,QUJD' } }));
    assert.deepEqual(await server.call('screenshot'), {
      content: [{ type: 'image', data: 'QUJD', mimeType: 'image/png' }],
    });
  });

  it('returns a CDP screenshot as a JPEG, not a mislabelled PNG', async () => {
    const { server } = tools(() => ({ ok: true, data: { screenshot: 'data:image/jpeg;base64,QUJD' } }));
    assert.deepEqual(await server.call('screenshot'), {
      content: [{ type: 'image', data: 'QUJD', mimeType: 'image/jpeg' }],
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

  it('lists elements with the ids analyze gives, scoped by a selector and limited', async () => {
    const data = {
      url: 'u',
      title: 'T',
      markdown: 'ignored',
      elements: [
        { id: 1, type: 'button', text: 'Go', visible: true },
        { id: 2, type: 'link', text: 'Home', visible: true },
      ],
    };
    const { server, driver } = tools(() => ({ ok: true, data }));
    const out = textOf(await server.call('read_elements', { selector: 'nav', limit: 1 }));
    assert.ok(out.includes('Page: T (u)\n\n## Element Index (1 total, 1 visible)'));
    assert.ok(out.includes('1,button,Go'));
    assert.ok(!out.includes('Home') && !out.includes('ignored'));
    assert.deepEqual([driver.sent[0].action, driver.sent[0].params], ['analyze', { selector: 'nav' }]);
  });

  it('lists tabs with the active one marked', async () => {
    const data = {
      tabs: [
        { id: 1, title: 'A', url: 'a', active: true },
        { id: 2, title: 'B', url: 'b' },
      ],
    };
    const { server } = tools(() => ({ ok: true, data }));
    assert.equal(textOf(await server.call('list_tabs')), 'Tabs:\n→ [tab 1] A, a\n  [tab 2] B, b');
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

  it('moves through history when navigate is given "back" or "forward"', async () => {
    const { server, driver } = tools(() => ({ ok: true }));
    assert.equal(textOf(await server.call('navigate', { url: 'back' })), 'Went back');
    assert.equal(textOf(await server.call('navigate', { url: 'forward' })), 'Went forward');
    assert.deepEqual(
      driver.sent.map((c) => c.action),
      ['back', 'forward'],
    );
  });

  it('switches to an open tab when open_tab is given its id', async () => {
    const { server, driver } = tools(() => ({ ok: true, data: {} }));
    assert.equal(textOf(await server.call('open_tab', { tab_id: 4 })), 'Switched to tab 4');
    assert.deepEqual([driver.sent[0].action, driver.sent[0].params], ['switch_tab', { tab_id: 4 }]);
  });

  it('refuses open_tab given both a URL and a tab, sending nothing', async () => {
    const { server, driver } = tools(() => ({ ok: true, data: {} }));
    const reply = await server.call('open_tab', { url: 'https://a.test', tab_id: 4 });
    assert.deepEqual([reply.isError, textOf(reply)], [true, 'Error: Give either url or tab_id, not both.']);
    assert.equal(driver.sent.length, 0);
  });
});

describe('browser tools for a chat connector', () => {
  beforeEach(() => stubControl());
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  /** A connector's server whose browser is acquired, with `note` said on the first reply. */
  const connector = (answer: (action: string, params: any) => any, note?: string) =>
    tools(answer, { pick: () => null, connector: true, acquire: async () => ({ id: B, note }) });

  it('runs on its acquired browser and leads the reply with the start note', async () => {
    const { server, driver } = connector(() => ({ ok: true }), 'Started a browser for this chat.');
    const reply = await server.call('navigate', { url: 'https://a.test' });
    assert.equal(textOf(reply), 'Started a browser for this chat.\nNavigated to https://a.test');
    assert.equal(driver.sent[0].action, 'navigate');
  });

  it('answers with the reason when no browser could be acquired', async () => {
    const acquire = async () => ({ error: 'The browser is still starting. Repeat this step shortly.' });
    const { server, driver } = tools(() => ({ ok: true }), { pick: () => null, connector: true, acquire });
    const reply = await server.call('click', { element_id: 1 });
    assert.deepEqual(
      [reply.isError, textOf(reply)],
      [true, 'Error: The browser is still starting. Repeat this step shortly.'],
    );
    assert.equal(driver.sent.length, 0);
  });

  it('cancels a dialog holding the page after a failed call, and reports it', async () => {
    const answer = (action: string) =>
      action === 'handle_dialog' ? { ok: true, data: {} } : { ok: false, error: 'The page asked "Leave?"' };
    const { server, driver } = connector(answer);
    const reply = await server.call('click', { element_id: 1 });
    assert.match(textOf(reply), /^Error: The page asked "Leave\?"\nOya dismissed it with Cancel\./);
    assert.deepEqual([driver.sent[1].action, driver.sent[1].params], ['handle_dialog', { accept: false }]);
  });

  it('never answers a dialog for a caller that is not a connector', async () => {
    const { server, driver } = tools(() => ({ ok: false, error: 'The page asked "Leave?"' }));
    assert.equal(textOf(await server.call('click', { element_id: 1 })), 'Error: The page asked "Leave?"');
    assert.deepEqual(
      driver.sent.map((c) => c.action),
      ['click'],
    );
  });

  it('lets only a call that opens a page start the browser: not a read, a click, a history move or a tab switch', async () => {
    const asked: Record<string, boolean> = {};
    const acquire = (label: string) => async (start: boolean) => ((asked[label] = start), { id: B });
    const run = async (label: string, name: string, args: object) => {
      const { server } = tools(() => ({ ok: true, data: {} }), {
        pick: () => null,
        connector: true,
        acquire: acquire(label),
      });
      await server.call(name, args);
    };
    await run('navigate', 'navigate', { url: 'https://a.test' });
    await run('open_tab', 'open_tab', { url: 'https://a.test' });
    await run('back', 'navigate', { url: 'back' });
    await run('switch', 'open_tab', { tab_id: 2 });
    await run('screenshot', 'screenshot', {});
    await run('click', 'click', { element_id: 1 });
    await run('wait_for', 'wait_for', { text: 'x' });
    assert.deepEqual(asked, {
      navigate: true,
      open_tab: true,
      back: false,
      switch: false,
      screenshot: false,
      click: false,
      wait_for: false,
    });
  });

  it('opens the live card beside open_tab, naming the browser it shows', async () => {
    const { server } = connector(() => ({ ok: true, data: { tab_id: 2 } }));
    assert.deepEqual(server.tools.get('open_tab')!.meta, { ui: { resourceUri: CARD_URI } });
    assert.equal(server.tools.get('navigate')!.meta, undefined);
    const reply = await server.call('open_tab', { url: 'https://a.test' });
    assert.deepEqual(reply.structuredContent, { browserId: B, name: 'Driven' });
  });

  it('shows no card on open_tab for other callers', async () => {
    const { server } = tools(() => ({ ok: true, data: { tab_id: 2 } }));
    assert.equal(server.tools.get('open_tab')!.meta, undefined);
    assert.equal((await server.call('open_tab', {})).structuredContent, undefined);
  });
});
