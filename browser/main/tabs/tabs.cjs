/**
 * The tabs: one BrowserView each, in the persona's partition, protected before
 * their first page loads. Opening, closing, switching, and the tab strip the
 * shell draws.
 */
const { wireTab } = require('./tab-events.cjs');
const { navigateActive, normalizeAddress } = require('./navigation.cjs');
const { HOME_URL, PAGE_BACKGROUND, ERR_ABORTED } = require('./constants.cjs');
const popups = require('./popup-tabs.cjs');

/** What the tab strip shows for one tab. */
function tabSummary(t, activeTabId) {
  return {
    id: t.id,
    title: t.title,
    url: t.url,
    active: t.id === activeTabId,
    ...loadSummary(t),
    ...historySummary(t.view.webContents.navigationHistory),
  };
}

/** Whether the tab is loading, and why it failed if it did. */
function loadSummary(t) {
  const loading = !!t.navigationPending || Boolean(t.view.webContents.isLoading?.());
  return { loading, loadError: t.loadError || null };
}

/** Whether Back and Forward can go anywhere. */
function historySummary(history) {
  if (!history) return { canGoBack: false, canGoForward: false };
  return { canGoBack: history.canGoBack(), canGoForward: history.canGoForward() };
}

/** Detach debugger before destroying. */
function destroyTabView(view) {
  try {
    if (view.webContents.debugger.isAttached()) view.webContents.debugger.detach();
  } catch {}
  try {
    if (!view.webContents.isDestroyed()) view.webContents.destroy();
  } catch {}
}

/** Closes every tab, then shows the welcome screen; nothing is reopened. */
function leaveBrowsing(tabs) {
  if (!tabs.ctx.shell.browsingMode) return;
  while (tabs.list.length) tabs.closeTab(tabs.list[0].id, { keepOne: false });
  tabs.ctx.shell.browsingMode = false;
  tabs.ctx.shell.send('mode-changed', 'setup');
}

/**
 * A first page that failed to load. One the person replaced by going somewhere
 * else first (ERR_ABORTED) is not a failure, and logging it cried wolf.
 */
function reportFirstLoad(e) {
  if (e.errno === ERR_ABORTED || e.code === 'ERR_ABORTED') return;
  console.error('[tab] Could not open page:', e.message);
}

/** The open tabs and which one is showing. */
class TabManager {
  /** `ctx` is the main-process context (see main.js). */
  constructor(ctx) {
    /** The main-process context. */
    this.ctx = ctx;
    /** @type {{ id: number, view: BrowserView, title: string, url: string }[]} */
    this.list = [];
    /** The tab on screen, or null. */
    this.activeTabId = null;
    /** The id the next tab gets. */
    this.nextTabId = 1;
  }

  /** The tab with this id, if open. */
  find(id) {
    return this.list.find((t) => t.id === id);
  }

  /** The active tab's view, or null. */
  getActiveView() {
    return this.find(this.activeTabId)?.view || null;
  }

  /** Puts a window the page opened on the tab list, so an agent can drive it. */
  adoptWindow(win) {
    return popups.adoptWindow(this, win);
  }

  /** Opens a tab on `url`, loaded with Electron's `loadOptions` (a referrer, a POST body); returns its id. */
  createTab(url, activate = true, loadOptions = undefined) {
    const tab = this.addTab(url);
    const tabReady = wireTab(this.ctx, tab);
    this.openFirstPage(tab, tabReady, { url, loadOptions });
    if (activate) this.activateTab(tab.id);
    this.sendTabList();
    return tab.id;
  }

  /** A new view in the persona's partition, on the list. */
  addTab(url) {
    const partition = this.ctx.persona.partitionName();
    const view = new this.ctx.electron.BrowserView({
      webPreferences: { contextIsolation: true, sandbox: true, partition },
    });
    // A page that sets no background of its own paints nothing, and the window's colour
    // shows through, in the dark theme that is dark text on a dark canvas. White is what
    // every other browser puts under a page; a page with its own background still wins.
    view.setBackgroundColor(PAGE_BACKGROUND);
    const tab = { id: this.nextTabId++, view, title: 'New Tab', url: url || '' };
    this.list.push(tab);
    return tab;
  }

  /** Loads the tab's first page once it is protected, unless a navigation already took over. */
  openFirstPage(tab, tabReady, { url, loadOptions }) {
    tab.setup = tabReady;
    const load = () => (url && !tab.navigationRequest ? tab.view.webContents.loadURL(url, loadOptions) : undefined);
    tab.ready = Promise.resolve(tabReady).then(load);
    tab.ready.catch(reportFirstLoad);
  }

  /** Shows a tab. A popup has a window of its own, so it is raised rather than mounted. */
  activateTab(id) {
    const tab = this.find(id);
    if (!tab || this.activeTabId === id) return;
    this.activeTabId = id;
    if (tab.window) return popups.raiseWindow(this, tab);
    this.showInShell(tab);
  }

  /** Mounts a tab's view on the shell window and tells the strip what is showing. */
  showInShell(tab) {
    if (!this.ctx.overlays.names.size) this.ctx.shell.window.setBrowserView(tab.view);
    this.ctx.layout.layoutActiveTab();
    this.ctx.shell.send('url-changed', tab.url);
    this.ctx.shell.send('title-changed', tab.title);
    this.sendTabList();
  }

  /** Moves `offset` tabs along the strip, wrapping. */
  cycleTab(offset) {
    const index = this.list.findIndex((tab) => tab.id === this.activeTabId);
    const target = this.list[(index + offset) % this.list.length];
    if (target) this.activateTab(target.id);
  }

  /** Closes a tab. `keepOne: false` lets a bulk close empty the list. */
  closeTab(id, { keepOne = true } = {}) {
    const idx = this.list.findIndex((t) => t.id === id);
    if (idx === -1) return;
    const wasActive = this.list[idx].id === this.activeTabId;
    this.removeTab(idx);
    this.afterClose(idx, wasActive, keepOne);
  }

  /** Takes a tab off the window, the list and any recording, and destroys its page. */
  removeTab(idx) {
    const tab = this.list[idx];
    this.list.splice(idx, 1);
    this.ctx.recorder.channels.forget(tab.view);
    if (tab.window) return popups.closeWindowTab(tab);
    try {
      this.ctx.shell.window.removeBrowserView(tab.view);
    } catch {}
    destroyTabView(tab.view);
  }

  /** Picks what shows after a close. */
  afterClose(idx, wasActive, keepOne) {
    if (this.list.length === 0) return this.closedLast(keepOne);
    if (!wasActive) return this.sendTabList();
    this.activeTabId = null;
    this.activateTab(this.list[Math.min(idx, this.list.length - 1)].id);
  }

  /** The last tab closed. */
  closedLast(keepOne) {
    this.activeTabId = null;
    // Only the user-facing close paths keep a window's worth of browser alive.
    // A bulk close (profile switch) wants the list actually empty, recreating
    // here made `while (tabs.length)` loop forever, spawning a renderer per turn.
    if (keepOne) this.createTab(HOME_URL, true);
    else this.sendTabList();
  }

  /** Sends the tab strip to the shell. */
  sendTabList() {
    this.ctx.shell.send(
      'tabs-updated',
      this.list.map((t) => tabSummary(t, this.activeTabId)),
    );
  }

  /** A tab's address changed. */
  urlChanged(tab, url) {
    tab.url = url;
    if (tab.id === this.activeTabId) this.ctx.shell.send('url-changed', url);
    this.sendTabList();
  }

  /** A tab's title changed. */
  titleChanged(tab, title) {
    tab.title = title;
    if (tab.id === this.activeTabId) this.ctx.shell.send('title-changed', title);
    this.sendTabList();
  }

  /** Leaves the setup screen and starts showing pages, on `url`. */
  enterBrowsingMode(url) {
    if (this.ctx.shell.browsingMode) return;
    this.ctx.shell.browsingMode = true;
    this.createTab(url || HOME_URL, true);
    this.ctx.shell.send('mode-changed', 'browsing');
  }

  /** Back to the welcome screen (log out): every tab closes and the shell shows setup. */
  leaveBrowsingMode() {
    leaveBrowsing(this);
  }

  /**
   * A tab for automation (the CDP front door, workflow validation). Outside
   * browsing mode a tab is never laid out, and a page with a 0x0 viewport is
   * both broken and an obvious bot.
   */
  openForAutomation(url) {
    if (this.ctx.shell.browsingMode) return this.createTab(url, true);
    this.enterBrowsingMode(url);
    return this.activeTabId;
  }

  /** The address bar (see navigation.cjs). */
  navigateActive(url) {
    return navigateActive(this.ctx, url);
  }

  /** Reload, or stop a load in progress. */
  reloadActivePage() {
    this.ctx.shield.requireHumanControl();
    const tab = this.find(this.activeTabId);
    if (!tab) return;
    if (tab.navigationPending || tab.view.webContents.isLoading()) this.stopLoading(tab);
    else this.reloadTab(tab);
  }

  /** Stops a load and forgets the navigation it was for. */
  stopLoading(tab) {
    tab.navigationRequest = (tab.navigationRequest || 0) + 1;
    tab.navigationPending = false;
    tab.view.webContents.stop();
    this.sendTabList();
  }

  /** Reloads a tab, clearing its error. */
  reloadTab(tab) {
    tab.loadError = null;
    tab.view.webContents.reload();
  }
}

module.exports = { TabManager, normalizeAddress };
