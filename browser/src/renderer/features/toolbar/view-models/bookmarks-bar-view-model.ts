/**
 * The bookmarks bar under the toolbar: whether it shows, and the profile's
 * bookmarks on it, following the main process (a bookmark added or removed, the
 * bar turned on or off from the View menu or Cmd/Ctrl+Shift+B).
 */
import type { LibraryKind } from '../../../../shared/library.ts';
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { Payload } from '../../../../shared/ipc.ts';
import { DownloadChipViewModel, type DownloadChipBridge } from './download-chip-view-model.ts';

/** One bookmark on the bar. */
export interface BarBookmark {
  /** Where it goes. */
  url: string;
  /** What it says. */
  title: string;
}

/** What the bar shows. */
export interface BookmarksBarState {
  /** Whether the bar shows. */
  shown: boolean;
  /** Every bookmarked address, for the toolbar's star. */
  saved: string[];
  /** Its bookmarks, newest first. */
  bookmarks: BarBookmark[];
}

/** The parts of the bridge the bar uses. */
export type BookmarksBarBridge = Pick<
  OyaBrowser,
  'bookmarksBar' | 'onBookmarksBar' | 'setBookmarksBar' | 'navigate' | 'newTab' | 'showLibrary' | 'toggleBookmark'
> &
  DownloadChipBridge;

/** The bar's state from what the main process sent; anything malformed shows nothing. */
function stateOf(payload: Payload | null | undefined): BookmarksBarState {
  const list = Array.isArray(payload?.bookmarks) ? (payload.bookmarks as BarBookmark[]) : [];
  const bookmarks = list
    .filter((b) => typeof b?.url === 'string')
    .map((b) => ({ url: b.url, title: b.title || b.url }));
  const saved = Array.isArray(payload?.saved) ? (payload.saved as unknown[]).filter((u) => typeof u === 'string') : [];
  return { shown: payload?.shown !== false, saved: saved as string[], bookmarks };
}

/** The bookmarks bar. */
export class BookmarksBarViewModel extends ViewModel<BookmarksBarState> {
  /** The main process. */
  private readonly bridge: BookmarksBarBridge;
  /** The Downloads button's chip, the same in either row the library buttons sit in. */
  readonly downloads: DownloadChipViewModel;

  /** Shown and empty until the main process says, then following it. */
  constructor(bridge: BookmarksBarBridge) {
    super({ shown: true, saved: [], bookmarks: [] });
    this.bridge = bridge;
    this.downloads = new DownloadChipViewModel(bridge);
    this.own(() => this.downloads.dispose());
    this.follow(bridge);
  }

  /** Follows the bar as the main process reports it, starting from what it says now. */
  private follow(bridge: BookmarksBarBridge): void {
    this.own(bridge.onBookmarksBar((payload) => this.set(stateOf(payload))));
    void bridge.bookmarksBar().then(
      (payload) => this.set(stateOf(payload)),
      () => {},
    );
  }

  /** Opens a bookmark here, or in a new tab with Cmd/Ctrl held, as browsers do. */
  open(url: string, newTab: boolean): void {
    void (newTab ? this.bridge.newTab(url) : this.bridge.navigate(url)).catch(() => {});
  }

  /** Bookmarks the page in the shown tab, or removes its bookmark: the toolbar's star. */
  toggleCurrent(): void {
    void this.bridge.toggleBookmark().catch(() => {});
  }

  /** Hides the bar (View > Always Show Bookmarks Bar brings it back). */
  hide(): void {
    void this.bridge.setBookmarksBar(false);
  }

  /** Opens or selects a collection tab. */
  showAll(kind: LibraryKind = 'bookmarks'): void {
    void this.bridge.showLibrary(kind);
  }
}
