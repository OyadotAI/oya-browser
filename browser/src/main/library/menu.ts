/** Native library menus stay usable above page views, with no renderer access to saved browsing data. */
import type { MenuItemConstructorOptions } from 'electron';
import { showLibrary } from './tabs.ts';
import type { AppServices } from '../app/services.ts';
import { CLEAR_HISTORY_CONFIRMATION, CONFIRM_RESPONSE } from './constants.ts';
import type { LibraryKind } from '../../shared/library.ts';
import { isIncognito } from '../incognito/index.ts';

/** The Forget item's label, counting the sites it would forget. */
function forgetLabel(count: number): string {
  return count ? `Forget site permissions (${count} ${count === 1 ? 'site' : 'sites'})` : 'Forget site permissions';
}

/** The native menu's collaborators. */
type Deps = Pick<
  AppServices,
  | 'windows'
  | 'appDir'
  | 'library'
  | 'downloads'
  | 'tabs'
  | 'electron'
  | 'shell'
  | 'control'
  | 'persona'
  | 'agentTab'
  | 'sitePermissions'
>;
/** Native history/bookmark navigation, shared by the toolbar and application menu. */
export class LibraryMenu {
  /** Services supplied by the composition root. */
  private readonly deps: Deps;
  /** Constructs a menu without keeping a stale persona snapshot. */
  constructor(deps: Deps) {
    this.deps = deps;
  }
  /** Whether the person may use the shown tab: with control, or beside an agent's tab. */
  private mayUse(): boolean {
    return this.deps.agentTab.humanMay(this.deps.tabs.activeTabId);
  }
  /** Opens the library with fresh contents, even after a profile switch. */
  show(kind: LibraryKind = 'history'): void {
    if (kind !== 'bookmarks' && kind !== 'history' && kind !== 'downloads') throw Error('Unknown collection');
    const scope = this.deps.windows?.fromContents(this.deps.shell.window!.webContents) ?? this.deps;
    if (scope.agentTab.humanMay(null)) showLibrary(scope, kind);
  }
  /** Native maintenance actions retain their existing confirmations. */
  manage(kind: LibraryKind = 'history'): void {
    if (!this.mayUse()) return;
    this.deps.electron.Menu.buildFromTemplate(this.items(kind)).popup({ window: this.deps.shell.window! });
  }
  /** The native list is built afresh for the current profile. */
  private items(kind: LibraryKind): MenuItemConstructorOptions[] {
    if (kind === 'downloads') return [this.clearDownloadsItem()];
    return kind === 'history'
      ? this.housekeeping(this.deps.library.snapshot().history.length).slice(1)
      : [this.forgetItem()];
  }
  /** The items that take something away: clearing confirms (irreversible), forgetting does not (the sites ask again). */
  private housekeeping(count: number): MenuItemConstructorOptions[] {
    return [
      { type: 'separator' },
      { label: 'Clear browsing history…', enabled: count > 0, click: () => void this.clear() },
      this.forgetItem(),
    ];
  }
  /** Forgets the sites this profile allowed a device on; no confirmation, since the sites simply ask again. The count is the feedback. */
  private forgetItem(): MenuItemConstructorOptions {
    const partition = this.deps.persona.partitionName();
    const count = this.deps.sitePermissions.origins(partition).length;
    const click = (): void => {
      if (this.mayUse() && partition === this.deps.persona.partitionName()) this.deps.sitePermissions.forget(partition);
    };
    return { label: forgetLabel(count), enabled: count > 0, click };
  }
  /** Clears the finished downloads off this profile's list; the files stay, so no confirmation. */
  private clearDownloadsItem(): MenuItemConstructorOptions {
    const count = this.deps.downloads.list().filter((d) => d.state !== 'progressing').length;
    return {
      label: 'Clear downloads list',
      enabled: count > 0,
      click: () => this.mayUse() && this.deps.downloads.clear(),
    };
  }
  /** Bookmarking uses the active page, not user-editable address-bar text; an incognito page leaves nothing in the library. */
  toggle(): void {
    if (!this.mayUse()) return;
    const tab = this.deps.tabs.find(this.deps.tabs.activeTabId);
    if (tab && !tab.home && !isIncognito(tab.view?.webContents?.session)) this.deps.library.toggle(tab.url, tab.title);
  }
  /** Confirmation cannot clear a different profile if the persona changes while it is open. */
  private async clear(): Promise<void> {
    const partition = this.deps.persona.partitionName();
    const result = await this.deps.electron.dialog.showMessageBox(this.deps.shell.window!, CLEAR_HISTORY_CONFIRMATION);
    if (result.response === CONFIRM_RESPONSE && partition === this.deps.persona.partitionName() && this.mayUse())
      this.deps.library.clearHistory();
  }
}
