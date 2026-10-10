/**
 * Unit tests for TabManager: opening, closing (and the keep-one rule a bulk
 * close opts out of), switching, reload/stop, and automation tabs.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { TabManager } from '../../../../src/main/tabs/tabs.ts';
import { HOME_URL, PAGE_BACKGROUND } from '../../../../src/main/tabs/constants.ts';
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

  it('stops a page that is still loading instead of reloading it', () => {
    const tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    tab.view.webContents.loading = true;
    ctx.tabs.reloadActivePage();
    assert.deepEqual(tab.view.webContents.calls, ['stop']);
    assert.equal(tab.navigationRequest, 1);
    tab.view.webContents.loading = false;
    tab.loadError = 'x';
    ctx.tabs.reloadActivePage();
    assert.deepEqual(tab.view.webContents.calls, ['stop', 'reload']);
    assert.equal(tab.loadError, null);
  });

  it('refuses a reload while an agent has control', () => {
    ctx.shield.requireHumanControl = () => {
      throw new Error('Take control');
    };
    assert.throws(() => ctx.tabs.reloadActivePage(), /Take control/);
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

  it('leaves browsing mode with every tab closed and none reopened', () => {
    ctx.shell.browsingMode = false;
    ctx.tabs.enterBrowsingMode();
    ctx.tabs.createTab('https://x.test/', true);
    ctx.tabs.leaveBrowsingMode();
    assert.equal(ctx.tabs.list.length, 0);
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
        active: true,
        loading: true,
        loadError: null,
        canGoBack: false,
        canGoForward: false,
      },
    ]);
  });
});
