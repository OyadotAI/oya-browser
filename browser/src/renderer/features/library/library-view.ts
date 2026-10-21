/** Searchable bookmarks and history in a theme-aware shell tab. */
import type { OyaBrowser } from '../../core/bridge.ts';
import {
  LIBRARY,
  type LibrarySnapshot,
  type LibraryKind,
  type LibraryAction,
  type LibraryPresentation,
} from '../../../shared/library.ts';
import { matching, sites, dayLabel, type LibrarySort } from './library-model.ts';
import { heading, emptyState } from './library-copy.ts';
import { text, row, button, downloadRow } from './library-dom.ts';
/** Library calls remain on the trusted shell bridge. */
type LibraryBridge = Pick<OyaBrowser, 'readLibrary' | 'libraryAction' | 'onBookmarksBar' | 'onShellCommand'>;
/** Filter state belongs to this window and never changes saved data. */
export class LibraryView {
  /** Cancels global listeners and fences late IPC responses on tab switches. */
  private readonly lifetime = new AbortController();
  /** Latest main-process snapshot for this one profile. */
  private data: LibrarySnapshot = { bookmarks: [], history: [], downloads: [] };
  /** Start with deliberately saved pages. */
  private kind: LibraryKind = 'bookmarks';
  /** Literal multiword search across title and URL. */
  private query = '';
  /** Empty means every website. */
  private site = '';
  /** Newest first by default. */
  private sort: LibrarySort = 'recent';
  /** Progressive rendering keeps even a full imported library responsive. */
  private limit: number = LIBRARY.PAGE_SIZE;
  /** The exact shell bridge, root and selected native tab. */
  private readonly bridge: LibraryBridge;
  /** Root scoped to this selected tab. */
  private readonly root: HTMLElement;
  /** Stable native tab identity. */
  private readonly tabId: number;
  /** Bind static controls once, then request profile data. */
  constructor(bridge: LibraryBridge, root: HTMLElement, tabId: number) {
    this.bridge = bridge;
    this.root = root;
    this.tabId = tabId;
    this.controls();
    void this.refresh().then(() => {
      if (!this.lifetime.signal.aborted) this.element('search').focus();
    });
  }
  /** A tab switch immediately removes listeners and invalidates outstanding reads. */
  dispose(): void {
    this.lifetime.abort();
  }
  /** Every selector is confined to this shell-owned collection. */
  private element(id: string): HTMLElement {
    return this.root.querySelector(`#${id}`)!;
  }
  /** Search and filters never require a server round trip. */
  private controls(): void {
    this.element('search').addEventListener('input', () => this.search());
    this.element('sort').addEventListener('change', () => this.sorting());
    this.element('more').addEventListener('click', () => this.more());
    this.element('manage').addEventListener('click', () => void this.action('manage'));
    this.element('clear-search').addEventListener('click', () => this.reset());
    this.events();
  }
  /** Window listeners are installed once, independently of rerendered rows. */
  private events(): void {
    const options = { signal: this.lifetime.signal };
    const forget = this.bridge.onBookmarksBar(() => void this.refresh());
    this.lifetime.signal.addEventListener('abort', forget, { once: true });
    // The main process takes Cmd/Ctrl+F before the page sees it, and hands it back here: find is the library's search.
    const unfind = this.bridge.onShellCommand((name) => name === 'find' && this.element('search').focus());
    this.lifetime.signal.addEventListener('abort', unfind, { once: true });
    this.root.addEventListener('keydown', (event) => this.key(event), options);
    window.addEventListener('focus', () => void this.refresh(), options);
  }
  /** Add one visible batch while searching always covers the full library. */
  private more(): void {
    this.limit += LIBRARY.PAGE_SIZE;
    this.render();
  }
  /** Refresh on focus picks up new visits, imports and native maintenance actions. */
  private async refresh(): Promise<void> {
    try {
      const data: LibraryPresentation = await this.bridge.readLibrary(this.tabId);
      if (this.lifetime.signal.aborted) return;
      this.present(data);
    } catch {
      this.element('status').textContent = 'Could not load this library. Close and reopen it to try again.';
    }
  }
  /** Apply a freshly verified native snapshot. */
  private present(data: LibraryPresentation): void {
    this.kind = data.kind;
    this.data = data;
    this.render();
  }
  /** Typing searches the whole collection, including pages beyond the visible batch. */
  private search(): void {
    this.query = (this.element('search') as HTMLInputElement).value;
    this.limit = LIBRARY.PAGE_SIZE;
    this.render();
  }
  /** Sorting never changes the order persisted in the library. */
  private sorting(): void {
    this.sort = (this.element('sort') as HTMLSelectElement).value as LibrarySort;
    this.limit = LIBRARY.PAGE_SIZE;
    this.render();
  }
  /** Empty results offer a single way back to all pages. */
  private reset(): void {
    (this.element('search') as HTMLInputElement).value = '';
    this.site = '';
    this.search();
    this.element('search').focus();
  }
  /** Esc clears the search (Cmd/Ctrl+F arrives as the main process's 'find' command, see events()). */
  private key(event: KeyboardEvent): void {
    if (event.key === 'Escape') this.reset();
  }
  /** Main verifies the source entry and profile again before any mutation or navigation. */
  private async action(action: LibraryAction, url?: string): Promise<void> {
    try {
      const data = await this.bridge.libraryAction(this.tabId, action, url);
      this.changed(data, action, url);
    } catch {
      this.element('status').textContent = this.failedText();
    }
  }
  /** Preserve keyboard focus after a bookmark change, including when its row disappears. */
  private changed(data: LibrarySnapshot | void, action: LibraryAction, url?: string): void {
    if (!data || this.lifetime.signal.aborted) return;
    this.data = data;
    this.render();
    if (action !== 'bookmark' && action !== 'remove') return;
    this.element('status').textContent = this.changedText(action);
    this.focusChanged(url);
  }
  /** What a failed action says. */
  private failedText(): string {
    if (this.kind === 'downloads') return 'That file was moved or deleted.';
    return 'That page could not be changed. Refresh the library and try again.';
  }
  /** What a change says to screen readers. */
  private changedText(action: LibraryAction): string {
    if (this.kind === 'downloads') return 'Removed from downloads';
    return action === 'bookmark' ? 'Bookmark saved' : 'Bookmark removed';
  }
  /** Keep a bookmark toggle or the next remaining page in the keyboard sequence. */
  private focusChanged(url?: string): void {
    const target = [...this.root.querySelectorAll<HTMLElement>('.page-row')].find((node) => node.dataset.url === url);
    const fallback = this.root.querySelector<HTMLElement>('.page-link') ?? this.element('search');
    (target?.querySelector<HTMLElement>('.save-page') ?? fallback).focus();
  }
  /** Summary, site filters and results stay aligned after every operation. */
  private render(): void {
    const pages = matching(this.data[this.kind], this.query, this.site, this.sort);
    heading(this.root, this.kind, this.data[this.kind].length, pages.length, this.site);
    this.siteFilters();
    this.results(pages);
    this.element('more').hidden = pages.length <= this.limit;
    this.element('empty').hidden = pages.length > 0;
    emptyState(this.root, this.kind, !!this.query.trim() || !!this.site);
  }
  /** Site navigation is derived locally from the selected collection. */
  private siteFilters(): void {
    const all = this.siteButton('', this.data[this.kind].length);
    const buttons = sites(this.data[this.kind]).map(([site, count]) => this.siteButton(site, count));
    this.element('sites').replaceChildren(all, ...buttons);
  }
  /** Native buttons expose selected state without relying on color alone. */
  private siteButton(site: string, count: number): HTMLElement {
    const node = button('', 'site-filter', () => this.filterSite(site));
    node.append(text('span', 'site-name', site || 'All websites'), text('span', 'count', String(count)));
    node.setAttribute('aria-pressed', String(site === this.site));
    node.title = site || 'All websites';
    node.dataset.site = site;
    return node;
  }
  /** Retain keyboard position after rebuilding the selected site filter. */
  private filterSite(site: string): void {
    this.site = site;
    this.limit = LIBRARY.PAGE_SIZE;
    this.render();
    [...this.root.querySelectorAll<HTMLElement>('.site-filter')].find((node) => node.dataset.site === site)?.focus();
  }
  /** Recent history has local-date headings; other sorts remain one easy-to-scan list. */
  private results(pages: LibrarySnapshot['history']): void {
    const pagesShown = pages.slice(0, this.limit);
    const content = document.createDocumentFragment();
    this.rows(pagesShown, content);
    this.element('results').replaceChildren(content);
  }
  /** Build a bounded list with date headings only for chronological history. */
  private rows(pages: LibrarySnapshot['history'], content: DocumentFragment): void {
    let group = '';
    pages.forEach((page) => {
      const label = this.kind !== 'bookmarks' && this.sort === 'recent' ? dayLabel(page.time) : '';
      if (label && label !== group) content.append(text('li', 'day-heading', label));
      group = label;
      content.append(this.pageRow(page));
    });
  }
  /** Bookmark controls always reflect the current snapshot, including in History. */
  private pageRow(page: LibrarySnapshot['history'][number]): HTMLElement {
    const act = (action: LibraryAction, key: string) => void this.action(action, key);
    if (this.kind === 'downloads') return downloadRow(page as LibrarySnapshot['downloads'][number], act);
    const saved = this.data.bookmarks.some((entry) => entry.url === page.url);
    return row(page, saved, (action, url) => void this.action(action, url));
  }
}
