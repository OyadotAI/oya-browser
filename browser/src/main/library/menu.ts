/** Native library menus stay usable above page views, with no renderer access to saved browsing data. */
import type { MenuItemConstructorOptions } from 'electron';
import type { AppServices } from '../app/services.ts';
import { safeAddress, type LibraryEntry } from './library.ts';
import { CLEAR_HISTORY_CONFIRMATION, CONFIRM_RESPONSE, MENU_PAGE_SIZE, MENU_TITLE_LENGTH } from './constants.ts';

/** The native menu's collaborators. */
type Deps = Pick<AppServices, 'library' | 'tabs' | 'electron' | 'shell' | 'control' | 'persona'>;
/** Native history/bookmark navigation, shared by the toolbar and application menu. */
export class LibraryMenu {
  /** Services supplied by the composition root. */
  private readonly deps: Deps;
  /** Constructs a menu without keeping a stale persona snapshot. */
  constructor(deps: Deps) {
    this.deps = deps;
  }
  /** Opens the library with fresh contents, even after a profile switch. */
  show(): void {
    if (!this.deps.control.snapshot().interactive) return;
    this.deps.electron.Menu.buildFromTemplate(this.items()).popup({ window: this.deps.shell.window! });
  }
  /** The native list is built afresh for the current profile. */
  private items(): MenuItemConstructorOptions[] {
    const { history, bookmarks } = this.deps.library.snapshot();
    return [
      this.bookmarkItem(),
      { label: 'Bookmarks', submenu: this.pages(bookmarks) },
      { label: 'History (recent pages)', submenu: this.pages(history) },
      { type: 'separator' },
      { label: 'Clear browsing history…', enabled: !!history.length, click: () => void this.clear() },
    ];
  }
  /** Bookmarking uses the active page, not user-editable address-bar text. */
  toggle(): void {
    if (!this.deps.control.snapshot().interactive) return;
    const tab = this.deps.tabs.find(this.deps.tabs.activeTabId);
    if (tab && !tab.home) this.deps.library.toggle(tab.url, tab.title);
  }
  /** Names the action honestly for the active page. */
  private bookmarkItem(): MenuItemConstructorOptions {
    const tab = this.deps.tabs.find(this.deps.tabs.activeTabId);
    const saved = this.deps.library.snapshot().bookmarks.some((entry) => entry.url === tab?.url);
    return {
      label: saved ? 'Remove bookmark for this page' : 'Bookmark this page',
      enabled: !!tab && !tab.home && safeAddress(tab.url),
      click: () => this.toggle(),
    };
  }
  /** Long lists are grouped instead of becoming an unmanageable single menu. */
  private pages(entries: LibraryEntry[]): MenuItemConstructorOptions[] {
    if (!entries.length) return [{ label: 'No pages yet', enabled: false }];
    if (entries.length <= MENU_PAGE_SIZE) return entries.map((entry) => this.page(entry));
    return Array.from({ length: Math.ceil(entries.length / MENU_PAGE_SIZE) }, (_value, index) => ({
      label: `${index * MENU_PAGE_SIZE + 1}–${Math.min((index + 1) * MENU_PAGE_SIZE, entries.length)}`,
      submenu: entries.slice(index * MENU_PAGE_SIZE, (index + 1) * MENU_PAGE_SIZE).map((entry) => this.page(entry)),
    }));
  }
  /** A saved title is plain text, with its host visible for disambiguation. */
  private page(entry: LibraryEntry): MenuItemConstructorOptions {
    const partition = this.deps.persona.partitionName();
    return {
      label: `${entry.title.slice(0, MENU_TITLE_LENGTH)} — ${new URL(entry.url).host}`.replaceAll('&', '&&'),
      click: () => {
        if (this.deps.control.snapshot().interactive && partition === this.deps.persona.partitionName())
          this.deps.tabs.createTab(entry.url, true);
      },
    };
  }
  /** Confirmation cannot clear a different profile if the persona changes while it is open. */
  private async clear(): Promise<void> {
    const partition = this.deps.persona.partitionName();
    const result = await this.deps.electron.dialog.showMessageBox(this.deps.shell.window!, CLEAR_HISTORY_CONFIRMATION);
    if (
      result.response === CONFIRM_RESPONSE &&
      partition === this.deps.persona.partitionName() &&
      this.deps.control.snapshot().interactive
    )
      this.deps.library.clearHistory();
  }
}
