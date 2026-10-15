/** IPC: the address bar, back/forward/reload, and the tab strip (open, close, switch, move, its menu). */
import { bookmarksBar, setBookmarksBar } from '../shell/bookmarks-bar.ts';
import type { AppServices } from '../app/services.ts';
import type { HandlersOf } from './handle.ts';
import { LibraryMenu, addressSuggestions, readLibrary, changeLibrary } from '../library/index.ts';
import { HOME_URL } from '../tabs/constants.ts';
import { moveTabTo } from '../tabs/tab-order.ts';
import { TabMenu } from '../tabs/tab-menu.ts';
import type { Tab } from '../tabs/types.ts';

/** The services navigation uses. */
type Deps = Pick<
  AppServices,
  | 'appDir'
  | 'externalApps'
  | 'shield'
  | 'shell'
  | 'tabs'
  | 'recorder'
  | 'control'
  | 'electron'
  | 'library'
  | 'downloads'
  | 'sitePermissions'
  | 'persona'
  | 'windows'
  | 'agentTab'
  | 'translation'
>;

/** The channels this group answers. */
type Channel =
  | 'address-suggestions'
  | 'show-library'
  | 'read-library'
  | 'library-action'
  | 'bookmarks-bar-state'
  | 'set-bookmarks-bar'
  | 'toggle-bookmark'
  | 'navigate'
  | 'go-back'
  | 'go-forward'
  | 'reload'
  | 'enter-browsing'
  | 'new-tab'
  | 'new-window'
  | 'detach-tab'
  | 'begin-tab-drag'
  | 'end-tab-drag'
  | 'close-tab'
  | 'activate-tab'
  | 'move-tab'
  | 'tab-menu'
  | 'translate-page'
  | 'show-original';

/** Navigation and the tab strip, as the shell asks for them. */
export class NavigationHandlers {
  /** Channel → handler. */
  readonly handlers: HandlersOf<Channel> = {
    'address-suggestions': (_e, query) => addressSuggestions(this.deps.library, query),
    'show-library': (_e, kind) => new LibraryMenu(this.deps).show(kind),
    'read-library': (_e, id) => readLibrary(this.deps, id),
    'library-action': (_e, id, action, url) => changeLibrary(this.deps, id, action, url),
    'bookmarks-bar-state': () => bookmarksBar(this.deps as AppServices),
    'set-bookmarks-bar': (_e, shown) => setBookmarksBar(this.deps as AppServices, shown === true),
    'toggle-bookmark': () => new LibraryMenu(this.deps).toggle(),
    navigate: (_e, url) => this.navigate(url),
    'go-back': () => this.goInHistory('go_back'),
    'go-forward': () => this.goInHistory('go_forward'),
    reload: () => this.deps.tabs.reloadActivePage(),
    'enter-browsing': () => this.deps.tabs.enterBrowsingMode(HOME_URL),
    'new-tab': (_e, url) => this.newTab(url),
    'new-window': () => this.deps.windows?.newWindow(),
    'detach-tab': (_e, id) => this.detach(id),
    'begin-tab-drag': (_e, id) => this.deps.windows?.drag.begin(this.deps as AppServices, id),
    'end-tab-drag': () => this.deps.windows?.drag.stop(this.deps as AppServices),
    'close-tab': (_e, id) => {
      this.deps.agentTab.requireFor(id);
      this.deps.tabs.closeTab(id);
    },
    'activate-tab': (_e, id) => this.deps.tabs.activateTab(id),
    // A drag on the strip: the main process owns the order, and the strip redraws from it.
    'move-tab': (_e, id, toIndex) => (this.deps.agentTab.requireFor(id), moveTabTo(this.deps.tabs, id, toIndex)),
    'tab-menu': (_e, id) => {
      this.deps.agentTab.requireFor(id);
      new TabMenu(this.deps).show(id);
    },
    'translate-page': (_e, id) => this.deps.translation.translate(this.ownTab(id)),
    'show-original': (_e, id) => this.deps.translation.showOriginal(this.ownTab(id)),
  };
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** The sender can only transfer a tab that still belongs to its own window. */
  private async detach(id: number): Promise<void> {
    this.deps.agentTab.requireFor(id);
    if (!Number.isSafeInteger(id) || !this.deps.tabs.find(id)) throw new Error('Tab is not in this window');
    try {
      await this.deps.windows?.detach(id, this.deps.electron.screen.getCursorScreenPoint());
    } finally {
      this.deps.windows?.drag.stop(this.deps as AppServices);
    }
  }

  /** The person's tab `id` in this window; throws when an agent holds it or it is not here. */
  private ownTab(id: number): Tab {
    this.deps.agentTab.requireFor(id);
    const tab = this.deps.tabs.find(id);
    if (!tab) throw new Error('Tab is not in this window');
    return tab;
  }

  /** Throws unless the person may use the tab their window shows: under the agent, any tab but the agent's. */
  private requireShownTab(): void {
    this.deps.agentTab.requireFor(this.deps.tabs.activeTabId);
  }

  /** Loads an address in the active tab, or leaves the start screen for it. */
  private async navigate(url: string): Promise<void> {
    this.requireShownTab();
    const contents = this.deps.tabs.getActiveView()?.webContents || this.deps.shell.window?.webContents;
    if (this.deps.externalApps.request(url, contents)) return;
    if (!this.deps.shell.browsingMode) return void this.deps.tabs.enterBrowsingMode(url);
    await this.deps.tabs.navigateActive(url);
  }

  /** Back or forward in the active tab, recorded as a step when a recording is running. */
  private async goInHistory(action: 'go_back' | 'go_forward'): Promise<void> {
    this.requireShownTab();
    const contents = this.deps.tabs.getActiveView()?.webContents;
    const back = action === 'go_back';
    const history = contents?.navigationHistory;
    if (!contents || (history && !(back ? history.canGoBack() : history.canGoForward()))) return;
    await this.deps.recorder.recordHistory(action);
    if (back) contents.goBack();
    else contents.goForward();
  }

  /** Opens a tab; the start page loads nothing, so a recording keeps only a real address. */
  private newTab(url: string | undefined): number {
    this.deps.agentTab.requireFor(null);
    const id = this.deps.tabs.createTab(url || HOME_URL, true);
    if (url) this.deps.recorder.recordNavigation(url);
    return id;
  }
}
