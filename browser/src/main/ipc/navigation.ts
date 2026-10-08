/** IPC: the address bar, back/forward/reload, and the tab strip (open, close, switch, move, its menu). */
import type { AppServices } from '../app/services.ts';
import type { HandlersOf } from './handle.ts';
import { LibraryMenu } from '../library/index.ts';
import { HOME_URL } from '../tabs/constants.ts';
import { moveTabTo } from '../tabs/tab-order.ts';
import { TabMenu } from '../tabs/tab-menu.ts';

/** The services navigation uses. */
type Deps = Pick<
  AppServices,
  'shield' | 'shell' | 'tabs' | 'recorder' | 'control' | 'electron' | 'library' | 'persona'
>;

/** The channels this group answers. */
type Channel =
  | 'show-library'
  | 'navigate'
  | 'go-back'
  | 'go-forward'
  | 'reload'
  | 'enter-browsing'
  | 'new-tab'
  | 'close-tab'
  | 'activate-tab'
  | 'move-tab'
  | 'tab-menu';

/** Navigation and the tab strip, as the shell asks for them. */
export class NavigationHandlers {
  /** Channel → handler. */
  readonly handlers: HandlersOf<Channel> = {
    'show-library': () => new LibraryMenu(this.deps).show(),
    navigate: (_e, url) => this.navigate(url),
    'go-back': () => this.goInHistory('go_back'),
    'go-forward': () => this.goInHistory('go_forward'),
    reload: () => this.deps.tabs.reloadActivePage(),
    'enter-browsing': () => this.deps.tabs.enterBrowsingMode(HOME_URL),
    'new-tab': (_e, url) => this.newTab(url),
    'close-tab': (_e, id) => {
      this.requireHuman();
      this.deps.tabs.closeTab(id);
    },
    'activate-tab': (_e, id) => this.deps.tabs.activateTab(id),
    // A drag on the strip: the main process owns the order, and the strip redraws from it.
    'move-tab': (_e, id, toIndex) => (this.requireHuman(), moveTabTo(this.deps.tabs, id, toIndex)),
    'tab-menu': (_e, id) => {
      this.requireHuman();
      new TabMenu(this.deps).show(id);
    },
  };
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Throws unless a person holds control. */
  private requireHuman(): void {
    this.deps.shield.requireHumanControl();
  }

  /** Loads an address in the active tab, or leaves the start screen for it. */
  private async navigate(url: string): Promise<void> {
    this.requireHuman();
    if (!this.deps.shell.browsingMode) return void this.deps.tabs.enterBrowsingMode(url);
    await this.deps.tabs.navigateActive(url);
  }

  /** Back or forward in the active tab, recorded as a step when a recording is running. */
  private async goInHistory(action: 'go_back' | 'go_forward'): Promise<void> {
    this.requireHuman();
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
    this.requireHuman();
    const id = this.deps.tabs.createTab(url || HOME_URL, true);
    if (url) this.deps.recorder.recordNavigation(url);
    return id;
  }
}
