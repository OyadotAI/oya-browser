/**
 * The navigation toolbar: the address bar (the active page's address as the
 * main process reports it, or what the person is typing, which a page changing
 * its own address never overwrites), Back, Forward and
 * Reload, the loading state of the active tab (progress, status, Reload
 * becoming Stop), and the page's title for the window. Other features ask
 * it to focus the address bar (the command palette's Cmd/Ctrl L).
 */
import type { LibraryKind } from '../../../../shared/library.ts';
import { BookmarksBarViewModel } from './bookmarks-bar-view-model.ts';
import { ShareChipViewModel, type ShareChipBridge } from './share-chip-view-model.ts';
import { FindBarViewModel, type FindBarBridge } from './find-bar-view-model.ts';
import { ZoomChipViewModel, type ZoomChipBridge } from './zoom-chip-view-model.ts';
import { AddressCompletion } from './address-completion.ts';
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { TabSummary } from '../../../../shared/ipc.ts';
import { TEXT } from '../model/constants.ts';

/** The active tab, as the toolbar reads it. */
export type ActiveTab = TabSummary & {
  /** Why its page failed (the main process's name for it). */
  loadError?: string | null;
};

/** The active tab's navigation state. */
export interface NavState {
  /** The page is loading. */
  loading: boolean;
  /** Why its last load failed, or ''. */
  error: string;
  /** Back has somewhere to go. */
  canGoBack: boolean;
  /** Forward has somewhere to go. */
  canGoForward: boolean;
}

/** What the toolbar shows. */
export interface ToolbarState {
  /** The address bar's text: the page's address, or what the person is typing. */
  url: string;
  /** The active page's address, apart from the text: the bookmark star toggles this page, whatever is typed. */
  page: string;
  /** The active page's title. */
  title: string;
  /** The active tab's navigation state. */
  nav: NavState;
  /** Counts requests to focus the address bar; the view focuses it on each change. */
  focusRequest: number;
}

/** How the address bar's status and the Reload button look. */
export interface NavLook {
  /** The status text. */
  status: string;
  /** The status's name for assistive tech. */
  statusLabel: string;
  /** The status's tooltip ('' for none). */
  statusTitle: string;
  /** Reload's icon (Stop while loading). */
  reloadIcon: 'reload' | 'close';
  /** Reload's name and tooltip. */
  reloadLabel: string;
}

/** The parts of the bridge the toolbar uses. */
export type ToolbarBridge = Pick<
  OyaBrowser,
  | 'addressSuggestions'
  | 'showOverlay'
  | 'hideOverlay'
  | 'showLibrary'
  | 'onDownloadBubble'
  | 'downloadAction'
  | 'navigate'
  | 'bookmarksBar'
  | 'onBookmarksBar'
  | 'setBookmarksBar'
  | 'toggleBookmark'
  | 'newTab'
  | 'goBack'
  | 'goForward'
  | 'reload'
  | 'getStatus'
  | 'onUrlChanged'
  | 'onTitleChanged'
  | 'onTabsUpdated'
> &
  ShareChipBridge &
  FindBarBridge &
  ZoomChipBridge;

/** Before any tab: nothing loading, nowhere to go. */
const IDLE: NavState = { loading: false, error: '', canGoBack: false, canGoForward: false };

/** The navigation state of the active tab in `tabs`. */
export function navOf(tabs: readonly ActiveTab[]): NavState {
  const tab = tabs.find((t) => t.active);
  if (!tab) return IDLE;
  const error = tab.loadError ?? tab.error ?? '';
  return { loading: !!tab.loading, error, canGoBack: !!tab.canGoBack, canGoForward: !!tab.canGoForward };
}

/** Reload at rest. */
const RELOAD = { reloadIcon: 'reload' as const, reloadLabel: TEXT.reload };

/** How the status and Reload look for `nav`. */
export function navLook({ loading, error }: NavState): NavLook {
  const status = error ? TEXT.failed : loading ? TEXT.loading : '';
  const reload = loading ? { reloadIcon: 'close' as const, reloadLabel: TEXT.stop } : RELOAD;
  return { status, statusLabel: error || status, statusTitle: error, ...reload };
}

/** The window's title for a page titled `title`. */
export const windowTitle = (title: string): string => (title ? title + TEXT.titleJoin + TEXT.app : TEXT.app);

/** The Ask button's classes for a panel `open` or not, while `recording` or not. */
export const askClass = (open: boolean, recording: boolean): string =>
  ['dev-btn agent-button', open && 'active', recording && 'recording'].filter(Boolean).join(' ');

/** The navigation toolbar. */
export class ToolbarViewModel extends ViewModel<ToolbarState> {
  /** The local address dropdown, with its own asynchronous lifecycle. */
  readonly completion: AddressCompletion;
  /** The bookmarks bar under the toolbar. */
  readonly bookmarks: BookmarksBarViewModel;
  /** The share chip: what a call is capturing, and the way back to it. */
  readonly share: ShareChipViewModel;
  /** The find bar under that (Cmd/Ctrl+F). */
  readonly find: FindBarViewModel;
  /** The address bar's zoom chip. */
  readonly zoom: ZoomChipViewModel;
  /** The main process. */
  private readonly bridge: ToolbarBridge;
  /** Last selected tab, to focus a new start page only once. */
  private activeId: number | undefined;
  /**
   * The field is the person's: from focus until Enter, blur or a tab switch. While it holds, a page changing
   * its own address (YouTube, Gmail) updates `page` and leaves the text, its selection and the suggestions alone.
   */
  private held = false;
  /** The person typed since focusing: blur then keeps their text, where a field only looked at catches up to the page. */
  private typed = false;
  /** A page address arrived, so the starting status, which may be older, is ignored. */
  private heard = false;

  /** Empty, following the active page; the address starts from the main process's status. */
  constructor(bridge: ToolbarBridge) {
    super({ url: '', page: '', title: '', nav: IDLE, focusRequest: 0 });
    this.bridge = bridge;
    this.completion = new AddressCompletion(bridge);
    this.bookmarks = new BookmarksBarViewModel(bridge);
    this.share = new ShareChipViewModel(bridge);
    this.find = new FindBarViewModel(bridge);
    this.zoom = new ZoomChipViewModel(bridge);
    this.follow(bridge);
  }

  /** The toolbar's own ViewModels, which end with it. */
  private parts(): { dispose(): void }[] {
    return [this.completion, this.bookmarks, this.share, this.find, this.zoom];
  }

  /** Follows the active page's address and title, and the tabs; the address starts from the status. Its parts end with it. */
  private follow(bridge: ToolbarBridge): void {
    this.own(() => this.parts().forEach((part) => part.dispose()));
    this.own(bridge.onUrlChanged((url) => ((this.heard = true), this.pageChanged(url))));
    this.own(bridge.onTitleChanged((title) => this.set({ title })));
    this.own(bridge.onTabsUpdated((tabs) => this.tabsChanged(tabs)));
    void bridge.getStatus().then((status) => status?.url && !this.heard && this.pageChanged(status.url));
  }

  /** The active page's address changed: shown, unless the person is typing over it. */
  private pageChanged(url: string): void {
    this.set({ page: url });
    if (!this.held) this.replace(url);
  }

  /** Puts `url` in the field from outside: what was being typed and its completion go. */
  private replace(url: string): void {
    this.held = this.typed = false;
    this.completion.dismiss();
    this.set({ url });
  }

  /** A newly selected start page is ready for an address, not an agent task. */
  private tabsChanged(tabs: readonly ActiveTab[]): void {
    const active = tabs.find((tab) => tab.active);
    const focus = active?.home && !active.collection && active.id !== this.activeId;
    // A switch shows the new tab's address, which its url-changed (sent just before) left in `page`.
    if (active?.id !== this.activeId && this.activeId !== undefined) this.replace(this.state.page);
    if (active?.id !== this.activeId) this.completion.dismiss();
    this.activeId = active?.id;
    this.set({ nav: navOf(tabs) });
    if (focus) this.focusAddress();
  }

  /** The person typed in the address bar. */
  edit(url: string, typing = false): void {
    this.held = this.typed = true;
    this.set({ url });
    void this.completion.search(url, typing);
  }

  /** Enter in the address bar: loads what it says (an address or a search). */
  submit(destination = this.completion.selectedUrl ?? this.state.url + this.completion.state.inline): void {
    this.held = this.typed = false;
    this.completion.dismiss();
    void this.bridge.navigate(destination.trim());
  }

  /** Escape: closes the suggestions first, as Chrome does; then puts the page's address back, selected, to type afresh. */
  escape(): void {
    if (this.completion.state.open) return this.completion.dismiss();
    this.replace(this.state.page);
    // Still focused: the restored address stays selected through the page's next change.
    this.held = true;
    this.focusAddress();
  }

  /** The field got focus: it is the person's until they leave it, so a page's address change cannot unselect it; suggestions open for what it shows. */
  focused(): void {
    this.held = true;
    void this.completion.search(this.state.url);
  }

  /** The field lost focus: typed text stays until the next address change; untouched, it shows the page's address now. */
  blurred(): void {
    const looked = this.held && !this.typed;
    this.held = this.typed = false;
    this.completion.dismiss();
    if (looked) this.set({ url: this.state.page });
  }

  /** Opens the native history and bookmarks menu above the page. */
  showLibrary(kind: LibraryKind = 'history'): void {
    void this.bridge.showLibrary(kind);
  }

  /** Back in the active tab. */
  back(): void {
    void this.bridge.goBack();
  }

  /** Forward in the active tab. */
  forward(): void {
    void this.bridge.goForward();
  }

  /** Reloads the active page (the button reads Stop while it loads; the main process decides). */
  reload(): void {
    void this.bridge.reload();
  }

  /** Focuses and selects the address bar (satisfies the command palette's `focusAddress`). */
  focusAddress(): void {
    this.set({ focusRequest: this.state.focusRequest + 1 });
  }
}
