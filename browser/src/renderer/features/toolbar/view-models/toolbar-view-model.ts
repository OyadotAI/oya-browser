/**
 * The navigation toolbar: the address bar (the active page's address as the
 * main process reports it, or what the person typed), Back, Forward and
 * Reload, the loading state of the active tab (progress, status, Reload
 * becoming Stop), and the page's title for the window. Other features ask
 * it to focus the address bar (the command palette's Cmd/Ctrl L).
 */
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
  /** The address bar's text. */
  url: string;
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
  | 'navigate'
  | 'goBack'
  | 'goForward'
  | 'reload'
  | 'getStatus'
  | 'onUrlChanged'
  | 'onTitleChanged'
  | 'onTabsUpdated'
>;

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
  /** The main process. */
  private readonly bridge: ToolbarBridge;
  /** Last selected tab, to focus a new start page only once. */
  private activeId: number | undefined;

  /** Empty, following the active page; the address starts from the main process's status. */
  constructor(bridge: ToolbarBridge) {
    super({ url: '', title: '', nav: IDLE, focusRequest: 0 });
    this.bridge = bridge;
    this.completion = new AddressCompletion(bridge);
    this.own(() => this.completion.dispose());
    this.own(bridge.onUrlChanged((url) => this.set({ url })));
    this.own(bridge.onTitleChanged((title) => this.set({ title })));
    this.own(bridge.onTabsUpdated((tabs) => this.tabsChanged(tabs)));
    void bridge.getStatus().then((status) => status?.url && this.set({ url: status.url }));
  }

  /** A newly selected start page is ready for an address, not an agent task. */
  private tabsChanged(tabs: readonly ActiveTab[]): void {
    const active = tabs.find((tab) => tab.active);
    const focus = active?.home && active.id !== this.activeId;
    if (active?.id !== this.activeId) this.completion.dismiss();
    this.activeId = active?.id;
    this.set({ nav: navOf(tabs) });
    if (focus) this.focusAddress();
  }

  /** The person typed in the address bar. */
  edit(url: string): void {
    this.set({ url });
    void this.completion.search(url);
  }

  /** Enter in the address bar: loads what it says (an address or a search). */
  submit(destination = this.completion.selectedUrl ?? this.state.url): void {
    this.completion.dismiss();
    void this.bridge.navigate(destination.trim());
  }

  /** Opens the native history and bookmarks menu above the page. */
  showLibrary(): void {
    void this.bridge.showLibrary();
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
