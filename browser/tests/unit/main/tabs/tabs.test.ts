/**
 * Unit tests for TabManager: opening, closing (and the keep-one rule a bulk
 * close opts out of), switching, reload/stop, and automation tabs.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { TabManager } from '../../../../src/main/tabs/tabs.ts';
import { HOME_URL, PAGE_BACKGROUND, TAB_PROBE_MS } from '../../../../src/main/tabs/constants.ts';
import { mainCtx } from '../../support/main-ctx.cjs';
import { flush } from '../../support/fakes.cjs';

describe('TabManager', () => {
  it('returns to the last selected tab instead of a neighbor after following a link', () => {
    const origin = ctx.tabs.createTab('https://origin.test/');
    ctx.tabs.createTab('https://unrelated.test/');
    ctx.tabs.activateTab(origin);
    const child = ctx.tabs.createTab('https://child.test/');
    ctx.tabs.closeTab(child);
    assert.equal(ctx.tabs.activeTabId, origin);
  });
  it('closing a background tab does not change focus or leave a stale return target', () => {
    const a = ctx.tabs.createTab('https://a.test/');
    const b = ctx.tabs.createTab('https://b.test/');
    const c = ctx.tabs.createTab('https://c.test/');
    ctx.tabs.closeTab(b);
    assert.equal(ctx.tabs.activeTabId, c);
    ctx.tabs.closeTab(c);
    assert.equal(ctx.tabs.activeTabId, a);
  });

  let ctx;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    ctx = mainCtx({ tabs: TabManager });
  });
  afterEach(() => mock.timers.reset());

  it('opens a tab in the persona partition on a white page and shows it', () => {
    const id = ctx.tabs.createTab('https://a.test/');
    const tab = ctx.tabs.find(id);
    assert.equal(tab.view.options.webPreferences.partition, 'persist:oya-browser');
    assert.equal(tab.view.options.webPreferences.preload, path.join(ctx.appDir, 'out', 'preload', 'recording.js'));
    assert.equal(tab.view.options.webPreferences.sandbox, true);
    assert.equal(tab.view.options.webPreferences.contextIsolation, true);
    assert.equal(tab.view.options.webPreferences.nodeIntegration, false);
    assert.equal(tab.view.options.webPreferences.nodeIntegrationInSubFrames, true);
    assert.equal(tab.view.background, PAGE_BACKGROUND);
    assert.equal(ctx.tabs.activeTabId, id);
    assert.deepEqual(ctx.shell.window.views, [tab.view]);
  });

  it('loads about:blank first and the page only after protection is set up', async () => {
    const order = [];
    ctx.protection.setupTabCDP = async () => (order.push('protected'), true);
    const tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    tab.view.webContents.loadImpl = async (url) => order.push(url);
    await tab.ready;
    assert.deepEqual(tab.view.webContents.loaded, ['about:blank', 'https://a.test/']);
    assert.deepEqual(order, ['protected', 'https://a.test/']);
  });

  it('never loads the page when protection never finishes: two bounded attempts, then the tab stays blank', async () => {
    ctx.protection.setupTabCDP = () => new Promise(() => {});
    const errors = mock.method(console, 'error', () => {});
    const tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    // Two attempts, each given its full time.
    await flush();
    mock.timers.tick(10000);
    await flush();
    mock.timers.tick(10000);
    await tab.ready.catch(() => {});
    assert.deepEqual(tab.view.webContents.loaded, ['about:blank']);
    assert.equal(tab.protection, 'failed');
    const lines = errors.mock.calls.map((c) => c.arguments[0]);
    assert.deepEqual(lines, [
      '[anonymity] tab protection did not finish on the first attempt, trying once more',
      '[anonymity] tab protection failed twice, tab not loaded',
    ]);
  });

  it("a new start page takes every page off the window, even an agent's stacked beneath (Electron's setBrowserView skips some)", () => {
    const page = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    const agent = ctx.tabs.find(ctx.tabs.createTab('https://b.test/', false));
    ctx.shell.window.views = [agent.view, page.view];
    ctx.tabs.createTab(HOME_URL);
    assert.deepEqual(ctx.shell.window.views, []);
  });

  it('a site opened from a tab keeps that tab’s panel, a new start page does not', () => {
    const inherited: Array<[number, string]> = [];
    ctx.layout.inherit = (id, url) => inherited.push([id, url]);
    const site = ctx.tabs.createTab('https://a.test/');
    ctx.tabs.createTab('https://b.test/', false);
    assert.deepEqual(inherited, [[site, 'https://a.test/']], 'only a tab that shows asks for the panel');
  });

  it('shows no web page on the start page, and brings the tab back with its first real page', () => {
    const tab = ctx.tabs.find(ctx.tabs.createTab(HOME_URL));
    assert.equal(tab.url, '', 'the address bar is empty on the start page');
    assert.deepEqual(ctx.shell.window.views, [], 'the shell draws the start page, so no view is mounted');
    assert.equal(ctx.tabs.getShownView(), null);
    ctx.tabs.urlChanged(tab, 'about:blank');
    ctx.tabs.titleChanged(tab, 'about:blank');
    assert.ok(tab.home, 'a blank page is not a real page');
    assert.deepEqual([tab.url, tab.title], ['', 'Oya'], 'the start page keeps its own name and an empty address');
    ctx.tabs.urlChanged(tab, 'https://a.test/');
    assert.ok(!tab.home);
    assert.deepEqual(ctx.shell.window.views, [tab.view]);
    assert.equal(ctx.shell.sentOn('url-changed').at(-1), 'https://a.test/');
  });

  it('keeps one tab open when the person closes the last', () => {
    const id = ctx.tabs.createTab('https://a.test/');
    ctx.tabs.closeTab(id);
    assert.equal(ctx.tabs.list.length, 1);
    assert.ok(ctx.tabs.list[0].home, 'the one left is on the start page');
  });

  it('takes a closed tab out of the recording', () => {
    const id = ctx.tabs.createTab('https://a.test/');
    const { view } = ctx.tabs.find(id);
    ctx.tabs.closeTab(id);
    assert.deepEqual(ctx.recorder.forgotten, [view]);
  });

  it('lets a bulk close empty the list', () => {
    ctx.tabs.createTab('https://a.test/');
    ctx.tabs.createTab('https://b.test/');
    while (ctx.tabs.list.length) ctx.tabs.closeTab(ctx.tabs.list[0].id, { keepOne: false });
    assert.equal(ctx.tabs.list.length, 0);
    assert.equal(ctx.tabs.activeTabId, null);
    assert.deepEqual(ctx.shell.sentOn('tabs-updated').at(-1), []);
  });

  it('native tab disposal never reads the debugger API', () => {
    ctx.nativeBrowsing = true;
    const tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    let reads = 0;
    Object.defineProperty(tab.view.webContents, 'debugger', {
      get() {
        reads++;
        throw new Error('Forbidden');
      },
    });
    ctx.tabs.closeTab(tab.id, { keepOne: false });
    assert.equal(reads, 0);
    assert.equal(tab.view.webContents.isDestroyed(), true);
  });

  it('destroys a closed tab without entering debugger cleanup', () => {
    const tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    tab.view.webContents.debugger.attach();
    ctx.tabs.closeTab(tab.id, { keepOne: false });
    assert.equal(tab.view.webContents.isDestroyed(), true);
    assert.equal(tab.view.webContents.debugger.isAttached(), true);
  });

  it('shows the neighbour when the active tab closes', () => {
    const a = ctx.tabs.createTab('https://a.test/');
    const b = ctx.tabs.createTab('https://b.test/');
    const c = ctx.tabs.createTab('https://c.test/');
    ctx.tabs.activateTab(b);
    ctx.tabs.closeTab(b);
    assert.equal(ctx.tabs.activeTabId, c);
    ctx.tabs.closeTab(c);
    assert.equal(ctx.tabs.activeTabId, a);
  });

  it('tells the find bar which tab is shown, so it never stays over another page', () => {
    const follow = mock.method(ctx.findBar, 'follow');
    const a = ctx.tabs.createTab('https://a.test/');
    const b = ctx.tabs.createTab('https://b.test/');
    ctx.tabs.activateTab(a);
    assert.deepEqual(
      follow.mock.calls.map((c) => c.arguments[0]),
      [a, b, a],
    );
  });

  it('ignores closing a tab that is not open', () => {
    ctx.tabs.createTab('https://a.test/');
    ctx.tabs.closeTab(99);
    assert.equal(ctx.tabs.list.length, 1);
  });

  it('does not put a tab over a raised overlay', () => {
    ctx.overlays.names.add('shell');
    ctx.tabs.createTab('https://a.test/');
    assert.deepEqual(ctx.shell.window.views, []);
  });

  it('cycles through tabs in both directions, wrapping', () => {
    const a = ctx.tabs.createTab('https://a.test/');
    const b = ctx.tabs.createTab('https://b.test/');
    ctx.tabs.cycleTab(1);
    assert.equal(ctx.tabs.activeTabId, a);
    ctx.tabs.cycleTab(ctx.tabs.list.length - 1);
    assert.equal(ctx.tabs.activeTabId, b);
  });

  it('reports url and title changes, and tells the shell only for the active tab', () => {
    const a = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    ctx.tabs.createTab('https://b.test/');
    ctx.tabs.urlChanged(a, 'https://a.test/next');
    ctx.tabs.titleChanged(a, 'A');
    assert.equal(a.url, 'https://a.test/next');
    assert.equal(a.title, 'A');
    assert.ok(!ctx.shell.sentOn('url-changed').includes('https://a.test/next'));
  });

  it('stops a page that is still loading instead of reloading it', async () => {
    const tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    tab.view.webContents.loading = true;
    await ctx.tabs.reloadActivePage();
    assert.deepEqual(tab.view.webContents.calls, ['probe', 'stop']);
    assert.equal(tab.navigationRequest, 1);
    tab.view.webContents.loading = false;
    tab.loadError = 'x';
    await ctx.tabs.reloadActivePage();
    assert.deepEqual(tab.view.webContents.calls, ['probe', 'stop', 'probe', 'reload']);
    assert.equal(tab.loadError, null);
  });

  describe('a person reloading a page that does not answer', () => {
    let tab;
    beforeEach(() => {
      tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
      tab.view.webContents.probeImpl = () => new Promise(() => {});
    });

    /** Clicks Reload and lets the page's answer time run out. */
    async function reloadHung() {
      const done = ctx.tabs.reloadActivePage();
      await flush();
      mock.timers.tick(TAB_PROBE_MS);
      await done;
    }

    it('ends its renderer and waits for it to be gone before reloading: reloading at once never loads', async () => {
      await reloadHung();
      assert.deepEqual(tab.view.webContents.calls, ['probe', 'crash']);
      assert.equal(tab.recovering, true);
    });

    it('ends the renderer of a page stuck loading in it, instead of stopping it', async () => {
      tab.view.webContents.loading = true;
      await reloadHung();
      assert.deepEqual(tab.view.webContents.calls, ['probe', 'crash']);
    });

    it('does not end the renderer of a page an agent claimed while it was being asked', async () => {
      const done = ctx.tabs.reloadActivePage();
      ctx.agentTab.humanMay = () => false;
      mock.timers.tick(TAB_PROBE_MS);
      await done;
      assert.deepEqual(tab.view.webContents.calls, ['probe']);
    });

    it('does not end the renderer of a tab closed while it was being asked', async () => {
      const done = ctx.tabs.reloadActivePage();
      ctx.tabs.closeTab(tab.id);
      mock.timers.tick(TAB_PROBE_MS);
      await done;
      assert.ok(!tab.view.webContents.calls.includes('crash'));
    });

    it('ends it once for a double click', async () => {
      const first = ctx.tabs.reloadActivePage();
      const second = ctx.tabs.reloadActivePage();
      mock.timers.tick(TAB_PROBE_MS);
      await Promise.all([first, second]);
      await ctx.tabs.reloadActivePage();
      assert.deepEqual(tab.view.webContents.calls, ['probe', 'crash']);
    });
  });

  it('never asks a patched-engine tab, whose preload cannot answer: a slow load there would read as a hang', async () => {
    const tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    tab.view.webContents.session = { _setOyaPreScriptPolicy: () => {} };
    await ctx.tabs.reloadActivePage();
    assert.deepEqual(tab.view.webContents.calls, ['reload']);
  });

  it('reloads a tab whose renderer already died, without asking it or ending it again', async () => {
    const tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    tab.view.webContents.crashed = true;
    await ctx.tabs.reloadActivePage();
    await ctx.tabs.reloadActivePage();
    assert.deepEqual(tab.view.webContents.calls, ['reload', 'reload']);
  });

  it('never ends the renderer of a page that answers', async () => {
    const tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    await ctx.tabs.reloadActivePage();
    assert.deepEqual(tab.view.webContents.calls, ['probe', 'reload']);
  });

  it('counts a page whose frame was replaced while it was asked as alive', async () => {
    const tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    tab.view.webContents.probeImpl = () => Promise.reject(new Error('Render frame was disposed'));
    await ctx.tabs.reloadActivePage();
    assert.deepEqual(tab.view.webContents.calls, ['probe', 'reload']);
  });

  it("refuses to reload the agent's page at once, and never asks or ends it", () => {
    const tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    ctx.control.state = { interactive: false, mode: 'agent', mine: false };
    assert.throws(() => ctx.tabs.reloadActivePage(), /Switch to Human Mode/);
    assert.deepEqual(tab.view.webContents.calls, []);
  });

  it('refuses a reload while an agent has control', () => {
    ctx.control.state = { interactive: false, mode: 'agent', mine: false };
    assert.throws(() => ctx.tabs.reloadActivePage(), /Switch to Human Mode/);
  });

  it('enters browsing mode once, with one tab', () => {
    ctx.shell.browsingMode = false;
    ctx.tabs.enterBrowsingMode();
    ctx.tabs.enterBrowsingMode('https://x.test/');
    assert.equal(ctx.tabs.list.length, 1);
    assert.ok(ctx.tabs.list[0].home);
    assert.deepEqual(ctx.shell.sentOn('mode-changed'), ['browsing']);
  });

  it('opens the Ask panel when pages first show on a site, and leaves the start page to its own task box', () => {
    let revealed = 0;
    ctx.layout.reveal = () => revealed++;
    ctx.shell.browsingMode = false;
    ctx.tabs.enterBrowsingMode();
    assert.equal(revealed, 0, 'the start page asks for the task itself');
    ctx.tabs.leaveBrowsingMode();
    ctx.tabs.enterBrowsingMode('https://a.test/');
    assert.equal(revealed, 1);
  });

  it('leaves browsing mode with every tab closed, none reopened, and the panel shut', () => {
    let shut = 0;
    ctx.layout.closeAll = () => shut++;
    ctx.shell.browsingMode = false;
    ctx.tabs.enterBrowsingMode();
    ctx.tabs.createTab('https://x.test/', true);
    ctx.tabs.leaveBrowsingMode();
    assert.equal(ctx.tabs.list.length, 0);
    assert.equal(shut, 1);
    assert.equal(ctx.shell.browsingMode, false);
    assert.deepEqual(ctx.shell.sentOn('mode-changed'), ['browsing', 'setup']);
  });

  it('opens automation tabs through browsing mode so they are laid out', () => {
    ctx.shell.browsingMode = false;
    const first = ctx.tabs.openForAutomation('https://a.test/');
    const second = ctx.tabs.openForAutomation('https://b.test/');
    assert.equal(ctx.shell.browsingMode, true);
    assert.deepEqual([first, second], [1, 2]);
  });

  it("marks no tab as the agent's in idle AI Mode, so the shown tab can still be dragged", () => {
    ctx.tabs.createTab('https://a.test/');
    ctx.control.state = { connected: true, mode: 'agent', mine: false, localClients: 0, interactive: false };
    ctx.tabs.sendTabList();
    assert.deepEqual(
      ctx.shell
        .sentOn('tabs-updated')
        .at(-1)
        .map((t) => t.agent),
      [false],
    );
  });

  it("marks the shown tab as the agent's while an agent holds the whole browser", () => {
    ctx.tabs.createTab('https://a.test/');
    ctx.control.state = { connected: false, mode: 'agent', mine: false, localClients: 1, interactive: false };
    ctx.tabs.sendTabList();
    assert.deepEqual(
      ctx.shell
        .sentOn('tabs-updated')
        .at(-1)
        .map((t) => t.agent),
      [true],
    );
  });

  it('marks only the tab an agent claimed while agents work in their own tabs', () => {
    const first = ctx.tabs.createTab('https://a.test/');
    ctx.tabs.createTab('https://b.test/');
    ctx.control.state = { connected: true, mode: 'agent', mine: false, localClients: 0, interactive: false };
    ctx.agentTab.claim(first);
    ctx.tabs.sendTabList();
    assert.deepEqual(
      ctx.shell
        .sentOn('tabs-updated')
        .at(-1)
        .map((t) => t.agent),
      [true, false],
    );
  });

  it('summarizes each tab for the strip', () => {
    const tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    tab.navigationPending = true;
    ctx.tabs.sendTabList();
    assert.deepEqual(ctx.shell.sentOn('tabs-updated').at(-1), [
      {
        id: 1,
        title: 'New Tab',
        url: 'https://a.test/',
        home: false,
        favicon: null,
        pinned: false,
        active: true,
        agent: false,
        sharing: false,
        loading: true,
        loadError: null,
        canGoBack: false,
        canGoForward: false,
      },
    ]);
  });

  it('a shared tab is listed with sharing: true, and every other tab false', () => {
    const shared = ctx.tabs.createTab('https://a.test/');
    ctx.tabs.createTab('https://b.test/');
    ctx.shareIndicator = { has: (id) => id === shared };
    ctx.tabs.sendTabList();
    const list = ctx.shell.sentOn('tabs-updated').at(-1);
    assert.deepEqual(
      list.map((t) => t.sharing),
      [true, false],
    );
  });
});
