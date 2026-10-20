/**
 * The tabs: one BrowserView each, in the persona's partition, protected before
 * their first page loads. Opening, closing, switching, and the tab strip the
 * shell draws.
 */
import type { BrowserView, BrowserWindow, LoadURLOptions, NavigationHistory, WebPreferences, Session } from 'electron';
import type { AppServices } from '../app/services.ts';
import { TabEvents, type TabEventsDeps } from './tab-events.ts';
import { AddressBar } from './navigation.ts';
import { HOME_URL, PAGE_BACKGROUND, ERR_ABORTED } from './constants.ts';
import * as popups from './popup-tabs.ts';
import { loadInTab, isUnprotected } from './load.ts';
import { isHome, shownViewOf } from './home.ts';
import { initialTab, mountTab, updateTabTitle, updateTabUrl } from './tab-lifecycle.ts';
import { TabSelection } from './tab-selection.ts';
import { ClosedTabs, reopenable } from './tab-order.ts';
import { recordingPreferences } from '../recording/preload.ts';
import { isNativeEngine } from '../native-policy/index.ts';
import type { Tab, TabView } from './types.ts';
import { reloadOrRecover } from './tab-recovery.ts';

export { normalizeAddress } from './navigation.ts';

/** The services the tabs use, their listeners' included. */
type Deps = TabEventsDeps &
  Pick<
    AppServices,
    | 'electron'
    | 'layout'
    | 'overlays'
    | 'cookies'
    | 'windows'
    | 'nativeBrowsing'
    | 'appDir'
    | 'agentTab'
    | 'shareIndicator'
    | 'findBar'
  >;

/** Optional native session is application-owned, never chosen by page-supplied partition strings. */
type CreateTabArguments = [url: string, activate?: boolean, loadOptions?: LoadURLOptions, session?: Session];

/** How a close treats the last tab. */
interface CloseOptions {
  /** False lets a bulk close (log out, profile switch) empty the list; a person's close keeps one tab. */
  keepOne?: boolean;
}

/** What the tab strip shows for one tab. */
export interface TabSummary {
  /** The tab's number. */
  id: number;
  /** Its title. */
  title: string;
  /** Its address. */
  url: string;
  /** Whether it is on the start page. */
  home: boolean;
  /** Local collection drawn by the shell, never inside the website view. */
  collection?: import('../../shared/library.ts').LibraryKind;
  /** Its icon as a data: URL, or null. */
  favicon: string | null;
  /** Whether it is the one on screen. */
  active: boolean;
  /** Whether it is pinned, so the strip draws it as an icon in the leading block. */
  pinned: boolean;
  /** Whether the agent drives it. */
  agent: boolean;
  /** Whether a call is capturing it. */
  sharing: boolean;
  /** Whether it is loading. */
  loading: boolean;
  /** Why its page failed, or null. */
  loadError: string | null;
  /** Whether Back goes anywhere. */
  canGoBack: boolean;
  /** Whether Forward goes anywhere. */
  canGoForward: boolean;
}

/** Per-tab marks that come from other services: which tabs the agent drives, which a call captures. */
interface TabMarks {
  /** Tabs the agent drives, or null for whichever tab is shown. */
  agentTabs: Array<number | null> | null;
  /** Whether a call is capturing the tab. */
  sharing: (id: number) => boolean;
}

/** Whether the agent drives the tab, and whether a call captures it. */
const markSummary = (t: Tab, marks: TabMarks): Pick<TabSummary, 'agent' | 'sharing'> => ({
  agent: !!marks.agentTabs?.includes(t.id),
  sharing: marks.sharing(t.id),
});

/** What the tab strip shows for one tab. */
function tabSummary(t: Tab, activeTabId: number | null, marks: TabMarks): TabSummary {
  return {
    id: t.id,
    ...placeSummary(t),
    active: t.id === activeTabId,
    ...markSummary(t, marks),
    ...loadSummary(t),
    ...historySummary(t.view.webContents.navigationHistory),
  };
}

/** Where the tab is: its title, address and icon, and whether it is on the start page. */
const placeSummary = (t: Tab): Pick<TabSummary, 'title' | 'url' | 'home' | 'favicon' | 'collection' | 'pinned'> => ({
  title: t.title,
  url: t.url,
  home: !!t.home,
  ...(t.collection ? { collection: t.collection } : {}),
  favicon: t.favicon || null,
  pinned: !!t.pinned,
});

/** Whether the tab is loading, and why it failed if it did. */
function loadSummary(t: Tab): Pick<TabSummary, 'loading' | 'loadError'> {
  const loading = !!t.navigationPending || Boolean(t.view.webContents.isLoading?.());
  return { loading, loadError: t.loadError || null };
}

/** Whether Back and Forward can go anywhere. */
function historySummary(history: NavigationHistory | undefined): Pick<TabSummary, 'canGoBack' | 'canGoForward'> {
  if (!history) return { canGoBack: false, canGoForward: false };
  return { canGoBack: history.canGoBack(), canGoForward: history.canGoForward() };
}

/** Destroy the native page without opening a debugging backend. */
function destroyTabView(view: TabView, _nativeBrowsing: boolean): void {
  try {
    // destroy() is on every webContents, though Electron's types leave it out.
    if (!view.webContents.isDestroyed()) (view.webContents as unknown as { destroy(): void }).destroy();
  } catch {}
}

/** How a first load failed, as Electron or loadInTab reports it. */
interface LoadFailure {
  /** Chromium's error number. */
  errno?: number;
  /** The error's code. */
  code?: string;
  /** What went wrong. */
  message?: string;
}

/**
 * A first page that failed to load. One the person replaced by going somewhere
 * else first (ERR_ABORTED) is not a failure, and logging it cried wolf.
 */
function reportFirstLoad(e: LoadFailure): void {
  if (e.errno === ERR_ABORTED || e.code === 'ERR_ABORTED' || isUnprotected(e)) return;
  console.error('[tab] Could not open page:', e.message);
}

/** Loads the tab's first page once it is protected, unless a navigation already took over. */
function openFirstPage(tab: Tab, tabReady: Promise<void>, url: string, loadOptions?: LoadURLOptions): void {
  tab.setup = tabReady;
  const load = (): Promise<void> | undefined =>
    url && !isHome(url) && !tab.navigationRequest ? loadInTab(tab, url, loadOptions) : undefined;
  tab.ready = Promise.resolve(tabReady).then(load);
  tab.ready.catch(reportFirstLoad);
}

/**
 * A tab's page settings. Background throttling is off so a tab keeps drawing while
 * the window is hidden or covered, and an agent's click is not left waiting for a
 * frame (see KEEP_RENDERING_SWITCHES in src/main/app/constants.ts).
 */
const tabPreferences = (partition: string, appDir: string, owner: Session): WebPreferences => ({
  ...recordingPreferences(appDir, !isNativeEngine(owner)),
  partition,
  backgroundThrottling: false,
});

/**
 * A person's close is remembered for Reopen closed tab; a bulk close (log
 * out, profile switch) forgets everything, so one profile never reopens
 * another's pages.
 */
function rememberClosed(tabs: TabManager, idx: number, keepOne: boolean): void {
  const tab = tabs.list[idx];
  if (!keepOne) return tabs.closed.clear();
  // An incognito window remembers its own closes in memory, and reopens them in its own session.
  if (reopenable(tab) || tab.view.webContents.session === tabs.privateSession) tabs.closed.push(tab.url, idx);
}

/** The last tab closed. */
function closedLast(tabs: TabManager, keepOne: boolean): void {
  tabs.activeTabId = null;
  // Only a person's close keeps one tab; a bulk close wants the list empty (recreating here looped forever).
  if (keepOne) tabs.createTab(HOME_URL, true);
  else tabs.sendTabList();
}

/** Stops a load and forgets the navigation it was for. */
function stopLoading(tabs: TabManager, tab: Tab): void {
  tab.navigationRequest = (tab.navigationRequest || 0) + 1;
  tab.navigationPending = false;
  tab.view.webContents.stop();
  tabs.sendTabList();
}

/** A single reload intent either stops the pending load or reloads the settled page. */
function reloadOrStop(tabs: TabManager, tab: Tab): void {
  if (tab.navigationPending || tab.view.webContents.isLoading()) stopLoading(tabs, tab);
  else tabs.reloadTab(tab);
}

/** Creates a page in the active partition, with white behind sites that paint no background. */
function createPageView(deps: Deps, session?: Session): BrowserView {
  const owner = session || deps.persona.session();
  deps.protection.assertSession(owner);
  const webPreferences = session
    ? { ...tabPreferences('', deps.appDir, owner), session }
    : tabPreferences(deps.persona.partitionName(), deps.appDir, owner);
  const view = new deps.electron.BrowserView({ webPreferences });
  view.setBackgroundColor(PAGE_BACKGROUND);
  return view;
}

/**
 * The tabs agents drive: the ones they claimed while agents work in their own tabs,
 * none while the person holds control, and null (whichever tab is shown) while an
 * agent holds the whole browser. Idle AI Mode claims nothing: marking the shown tab
 * there made the strip refuse to drag it, so a one-tab window could never be moved.
 */
function agentTabs({ agentTab, control }: Pick<AppServices, 'agentTab' | 'control'>): number[] | null {
  if (agentTab.scoped()) return agentTab.claimed();
  return control.snapshot().interactive ? [] : null;
}

/** The strip's marks from the agent's claims and the live tab shares. */
const tabMarks = (deps: Deps): TabMarks => ({
  agentTabs: agentTabs(deps),
  sharing: (id) => !!deps.shareIndicator?.has(id),
});

/** Hidden transfer shells render metadata without owning a second copy of the page. */
function windowTabSummaries(tabs: TabManager, marks: TabMarks, staged?: Tab) {
  const visible = tabs.list.length ? tabs.list : staged ? [staged] : [];
  const active = tabs.activeTabId ?? staged?.id ?? null;
  return visible.map((tab) => tabSummary(tab, active, { ...marks, agentTabs: marks.agentTabs ?? [active] }));
}
/** The events a new tab is wired to: its window's, when the app has several, else this manager's own. */
function eventsFor(deps: AppServices, tab: Tab, own: TabEvents): TabEvents {
  const scope = deps.windows?.forTab(tab, deps);
  return scope ? new TabEvents(scope) : own;
}

/** The open tabs and which one is showing. */
export class TabManager {
  /** The main-process services the tabs use. */
  private readonly deps: Deps;
  /** Wires each new tab's listeners and protection. */
  private readonly events: TabEvents;
  /** The open tabs, in strip order. */
  list: Tab[] = [];
  /** The tab on screen, or null. */
  activeTabId: number | null = null;
  /** The id the next tab gets. */
  nextTabId = 1;
  /** Tabs a person closed, for Reopen closed tab. */
  readonly closed = new ClosedTabs();
  /** Recently selected tabs, newest last, independent of strip order. */
  private readonly selection = new TabSelection();
  /** An incognito window's session: every tab it opens without one lands here, never in the profile. */
  readonly privateSession?: Session;

  /** `deps` are the main-process services (see services.ts); `privateSession` makes the window incognito. */
  constructor(deps: Deps, privateSession?: Session) {
    this.deps = deps;
    this.events = new TabEvents(deps);
    this.privateSession = privateSession;
  }

  /** The tab with this id, if open. */
  find(id: number | null | undefined): Tab | undefined {
    return this.list.find((t) => t.id === id);
  }

  /** The active tab's view, or null. */
  getActiveView(): TabView | null {
    return this.find(this.activeTabId)?.view || null;
  }

  /** The active tab's view as the window shows it: null while that tab is on the start page. */
  getShownView(): TabView | null {
    return shownViewOf(this.find(this.activeTabId));
  }

  /** Puts a window the page opened on the tab list, so an agent can drive it. */
  adoptWindow(win: BrowserWindow): number | null {
    return popups.adoptWindow(this, win, this.deps.windows?.allocateTabId());
  }

  /** Opens a tab on `url`, loaded with Electron's `loadOptions` (a referrer, a POST body); returns its id. */
  createTab(...args: CreateTabArguments): number {
    const [url, activate = true, loadOptions, session] = args;
    const tab = this.addTab(url, session ?? this.privateSession);
    const tabReady = eventsFor(this.deps as AppServices, tab, this.events).wire(tab);
    openFirstPage(tab, tabReady, url, loadOptions);
    if (activate) this.deps.layout.inherit(tab.id, url);
    if (activate) this.activateTab(tab.id);
    this.sendTabList();
    return tab.id;
  }

  /** A new view in the persona's partition, on the list. */
  private addTab(url: string, session?: Session): Tab {
    const id = this.deps.windows?.allocateTabId() ?? this.nextTabId++;
    const tab = initialTab(id, createPageView(this.deps, session), url);
    this.list.push(tab);
    return tab;
  }

  /** Shows a tab. A popup has a window of its own, so it is raised rather than mounted. */
  activateTab(id: number): void {
    const tab = this.find(id);
    if (!tab || this.activeTabId === id) return;
    this.activeTabId = this.selection.visit(id);
    if (tab.window) return popups.raiseWindow(this, tab);
    this.showInShell(tab);
  }

  /** Mounts a tab's view on the shell window (unless an overlay holds it) and tells the strip what is showing. */
  showInShell(tab: Tab): void {
    mountTab(this.deps, tab);
  }

  /** Remove ownership without destroying the live page or touching its recording channel. */
  releaseTab(id: number): void {
    const index = this.list.findIndex((tab) => tab.id === id);
    if (index < 0) return;
    const [tab] = this.list.splice(index, 1);
    this.deps.shell.window?.removeBrowserView(tab.view as BrowserView);
    this.selection.forget(id);
    this.afterClose(index, this.activeTabId === id, false);
  }

  /** Adopt the same view and id into this window, preserving history, forms and renderer state. */
  receiveTab(tab: Tab): void {
    this.list.push(tab);
    this.activateTab(tab.id);
  }

  /** Moves along the strip, independently of the most-recent return order. */
  cycleTab(offset: number): void {
    const target = this.selection.cycle(this.list, this.activeTabId, offset);
    if (target !== undefined) this.activateTab(target);
  }

  /** Closes a tab. `keepOne: false` lets a bulk close empty the list. */
  closeTab(id: number, { keepOne = true }: CloseOptions = {}): void {
    const idx = this.list.findIndex((t) => t.id === id);
    if (idx === -1) return;
    const wasActive = this.list[idx].id === this.activeTabId;
    rememberClosed(this, idx, keepOne);
    this.removeTab(idx);
    this.selection.forget(id);
    this.afterClose(idx, wasActive, keepOne);
  }

  /** Takes a tab off the window, the list and any recording, and destroys its page. */
  private removeTab(idx: number): void {
    const tab = this.list[idx];
    this.list.splice(idx, 1);
    this.deps.recorder.channels.forget(tab.view);
    if (tab.window) return popups.closeWindowTab(tab);
    try {
      this.deps.shell.window!.removeBrowserView(tab.view as BrowserView);
    } catch {}
    destroyTabView(tab.view, !!this.deps.nativeBrowsing);
  }

  /** Picks what shows after a close. */
  private afterClose(idx: number, wasActive: boolean, keepOne: boolean): void {
    if (this.list.length === 0) return closedLast(this, keepOne);
    if (!wasActive) return this.sendTabList();
    this.activeTabId = null;
    this.activateTab(this.selection.previous(this.list) ?? this.list[Math.min(idx, this.list.length - 1)].id);
  }

  /** Sends the tab strip to the shell. */
  sendTabList(): void {
    this.deps.shell.send('tabs-updated', windowTabSummaries(this, tabMarks(this.deps), this.deps.shell.stagedTab));
    this.deps.windows?.tabEvents?.changed();
  }

  /** A tab's address changed. */
  urlChanged(tab: Tab, url: string): void {
    updateTabUrl(this, this.deps.shell, tab, url);
  }

  /** A tab's title changed. */
  titleChanged(tab: Tab, title: string): void {
    updateTabTitle(this, this.deps.shell, tab, title);
  }

  /** Leaves the setup screen and starts showing pages, on `url`: a site opens with Ask beside it, the start page with only its own task box (the panel opens with the first task). */
  enterBrowsingMode(url?: string): void {
    if (this.deps.shell.browsingMode) return;
    this.deps.shell.browsingMode = true;
    this.createTab(url || HOME_URL, true);
    this.deps.shell.send('mode-changed', 'browsing');
    if (!isHome(url || HOME_URL)) this.deps.layout.reveal();
  }

  /** Back to the welcome screen (log out): every tab closes, nothing is reopened, and the shell shows setup. */
  leaveBrowsingMode(): void {
    if (!this.deps.shell.browsingMode) return;
    while (this.list.length) this.closeTab(this.list[0].id, { keepOne: false });
    this.deps.layout.closeAll();
    this.deps.shell.browsingMode = false;
    this.deps.shell.send('mode-changed', 'setup');
  }

  /**
   * A tab for automation (the CDP front door, workflow validation). Outside browsing
   * mode a tab is never laid out, and a page with a 0x0 viewport is broken and an obvious bot.
   */
  openForAutomation(url: string): number | null {
    if (this.deps.shell.browsingMode) return this.createTab(url, true);
    this.enterBrowsingMode(url);
    return this.activeTabId;
  }

  /** The address bar (see navigation.ts). */
  navigateActive(url: string): Promise<void> {
    return new AddressBar(this.deps).navigate(url);
  }

  /** The person's Reload: recovers a stuck page (tab-recovery.ts), else reloads, or stops a load in progress. */
  reloadActivePage(): Promise<void> {
    this.deps.agentTab.requireFor(this.activeTabId); // refuses at once, before any wait
    const tab = this.find(this.activeTabId);
    return tab ? reloadOrRecover(this.deps, tab, () => reloadOrStop(this, tab)) : Promise.resolve();
  }

  /** Reloads a tab, clearing its error; one that could not be protected keeps saying so (only about:blank reloads). */
  reloadTab(tab: Tab): void {
    if (tab.protection !== 'failed') tab.loadError = null;
    tab.view.webContents.reload();
  }
}
