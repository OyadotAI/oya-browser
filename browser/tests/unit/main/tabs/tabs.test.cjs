/**
 * Unit tests for TabManager: opening, closing (and the keep-one rule a bulk
 * close opts out of), switching, reload/stop, and automation tabs.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { TabManager } = require('../../../../main/tabs/tabs.cjs');
const { HOME_URL, PAGE_BACKGROUND } = require('../../../../main/tabs/constants.cjs');
const { mainCtx } = require('../../support/main-ctx.cjs');
const { flush } = require('../../support/fakes.cjs');

describe('TabManager', () => {
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

  it('keeps one tab open when the person closes the last', () => {
    const id = ctx.tabs.createTab('https://a.test/');
    ctx.tabs.closeTab(id);
    assert.equal(ctx.tabs.list.length, 1);
    assert.equal(ctx.tabs.list[0].url, HOME_URL);
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

  it('destroys a closed tab and detaches its debugger', () => {
    const tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    tab.view.webContents.debugger.attach();
    ctx.tabs.closeTab(tab.id, { keepOne: false });
    assert.equal(tab.view.webContents.isDestroyed(), true);
    assert.equal(tab.view.webContents.debugger.isAttached(), false);
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
    assert.equal(ctx.tabs.list[0].url, HOME_URL);
    assert.deepEqual(ctx.shell.sentOn('mode-changed'), ['browsing']);
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
        active: true,
        loading: true,
        loadError: null,
        canGoBack: false,
        canGoForward: false,
      },
    ]);
  });
});
