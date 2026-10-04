/**
 * The tab strip's models: a tab as the main process reports it, an item on
 * the strip (a tab, possibly folding away or growing in), and the pure rules
 * for what an item shows and how a new tab list merges into the strip.
 */
import type { TabSummary } from '../../../../shared/ipc.ts';
import { TEXT, WEB_SCHEME } from './constants.ts';

/**
 * A tab as the main process sends it. The main process names its load error
 * `loadError` (src/main/tabs/tabs.ts); the shared TabSummary says `error`.
 */
export type Tab = TabSummary & {
  /** Why its page failed, as the main process sends it. */
  loadError?: string | null;
};

/** One item on the strip. */
export interface TabItemModel {
  /** The tab it draws. */
  tab: Tab;
  /** It was closed and is folding away; it is no longer a tab. */
  closing: boolean;
  /** It opened after the strip first drew, and grows in. */
  opening: boolean;
}

/** A favicon to show. */
export interface FaviconIcon {
  /** Its data: URL. */
  favicon: string;
}

/** What a tab's icon shows: a spinner, the Oya mark, the globe, or a favicon. */
export type IconKind = 'loading' | 'home' | 'globe' | FaviconIcon;

/** Why the tab's last load failed, or null. */
export const loadError = (tab: Tab): string | null => tab.loadError ?? tab.error ?? null;

/** The tab's icon: a spinner while it loads, the Oya mark at home, its favicon, else the globe. */
export function iconKind(tab: Tab): IconKind {
  if (tab.loading) return 'loading';
  if (tab.home) return 'home';
  return tab.favicon ? { favicon: tab.favicon } : 'globe';
}

/** The tab button's text. */
export const tabTitle = (tab: Tab): string => tab.title || TEXT.untitled;

/** The close button's name. */
export const closeLabel = (tab: Tab): string => (tab.title ? TEXT.close + tab.title : TEXT.closeUntitled);

/** What the hover card says the tab shows: its address without the scheme, or where it is. */
export function cardAddress(tab: Tab): string {
  if (tab.home) return TEXT.home;
  return String(tab.url || '').replace(WEB_SCHEME, '') || TEXT.blank;
}

/** How a new tab list merges into the strip. */
export interface MergeOptions {
  /** A press on a tab is under way: the shown order holds until it ends. */
  holdOrder: boolean;
  /** New tabs grow in (the strip has drawn before and motion is allowed). */
  animateNew: boolean;
  /** Closed tabs fold away rather than going at once. */
  fold: boolean;
}

/** The open items for `tabs`, in the main process's order, or the shown one while held. */
function openItems(prev: readonly TabItemModel[], tabs: readonly Tab[], options: MergeOptions): TabItemModel[] {
  const known = new Map(prev.filter((item) => !item.closing).map((item) => [item.tab.id, item]));
  const items = tabs.map((tab) => ({ tab, closing: false, opening: known.get(tab.id)?.opening ?? options.animateNew }));
  if (!options.holdOrder) return items;
  const rank = (id: number) => [...known.keys()].indexOf(id);
  const shown = items.filter((item) => known.has(item.tab.id)).sort((a, b) => rank(a.tab.id) - rank(b.tab.id));
  return [...shown, ...items.filter((item) => !known.has(item.tab.id))];
}

/** The strip's items once `tabs` arrive: open ones updated, closed ones folding where they stood. */
export function mergeItems(prev: readonly TabItemModel[], tabs: readonly Tab[], options: MergeOptions): TabItemModel[] {
  const ids = new Set(tabs.map((tab) => tab.id));
  const items = openItems(prev, tabs, options);
  if (!options.fold) return items;
  prev.forEach((item, index) => {
    if (!ids.has(item.tab.id)) items.splice(Math.min(index, items.length), 0, { ...item, closing: true });
  });
  return items;
}

/** The items with tab `id` moved to `to` among the open ones (closing ones keep their places). */
export function moveItem(items: readonly TabItemModel[], id: number, to: number): TabItemModel[] {
  const open = items.filter((item) => !item.closing);
  const moving = open.find((item) => item.tab.id === id);
  if (!moving) return [...items];
  const rest = open.filter((item) => item !== moving);
  rest.splice(to, 0, moving);
  return items.map((item) => (item.closing ? item : (rest.shift() as TabItemModel)));
}
