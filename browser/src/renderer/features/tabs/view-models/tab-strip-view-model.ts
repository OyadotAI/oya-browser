/**
 * The tab strip: one item per open page, kept in the main process's order,
 * sized like Chrome's (an even share of the strip, at most 240px, scrolling
 * once tabs reach their narrowest). Tabs fold away when closed, and after a
 * close with the mouse their widths hold until the pointer leaves the strip,
 * so the next close button lands under it. A drag's drop reorders the strip
 * at once; if the main process refuses the move, its own order comes back.
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import { mergeItems, moveItem, type Tab, type TabItemModel } from '../model/tab-model.ts';
import { tabSize, tabWidth, type TabSize } from '../model/tab-math.ts';
import { isRovingKey, rovingIndex } from '../../../hooks/index.ts';
import type { Drop, TabDragViewModel } from './tab-drag-view-model.ts';
import type { TabCardViewModel } from './tab-card-view-model.ts';

/** What the strip shows. */
export interface TabStripState {
  /** The tabs as the main process last sent them. */
  tabs: Tab[];
  /** The strip's items in shown order, with those folding away. */
  items: TabItemModel[];
  /** The width the tabs may share, as the view measured it. */
  room: number;
  /** A tab width held after a close with the mouse, until the pointer leaves the strip; else null. */
  frozen: number | null;
  /** The last drop, for the view to settle the slides and glide the tab home; null before any. */
  drop: Drop | null;
}

/** How the tabs are laid out. */
export interface StripLayout {
  /** Each tab's width (--tab-width). */
  width: number;
  /** The size they draw at (data-size). */
  size: TabSize;
  /** The tabs overflow the room, so the strip scrolls. */
  overflowing: boolean;
}

/** The parts of the bridge the strip uses. */
export type TabStripBridge = Pick<
  OyaBrowser,
  'onTabsUpdated' | 'newTab' | 'closeTab' | 'activateTab' | 'moveTab' | 'showTabMenu'
>;

/** What the strip uses. */
export interface TabStripDeps {
  /** The main process. */
  bridge: TabStripBridge;
  /** The hover card, hidden by a close or a menu. */
  card: Pick<TabCardViewModel, 'hide'>;
  /** The drag, which holds the order while a press lasts and answers drops. */
  drag: Pick<TabDragViewModel, 'state' | 'release'>;
  /** Whether the person asked for less motion (no folding or growing). */
  reducedMotion: () => boolean;
}

/** The layout for `state`: a held width, or an even share of the room. */
export function stripLayout({ tabs, room, frozen }: TabStripState): StripLayout {
  const width = frozen ?? tabWidth(room, tabs.length);
  return { width, size: tabSize(width), overflowing: tabs.length * width > room };
}

/** The tab strip. */
export class TabStripViewModel extends ViewModel<TabStripState> {
  /** What it uses. */
  private readonly deps: TabStripDeps;
  /** Whether the strip has drawn once: tabs that open later grow in. */
  private drawn = false;
  /** The timers that take folded tabs away. */
  private readonly folds = new Set<ReturnType<typeof setTimeout>>();

  /** Empty, following the main process's tab list. */
  constructor(deps: TabStripDeps) {
    super({ tabs: [], items: [], room: 0, frozen: null, drop: null });
    this.deps = deps;
    this.own(deps.bridge.onTabsUpdated((tabs) => this.update(tabs)));
    this.own(() => this.folds.forEach(clearTimeout));
  }

  /** Brings the strip in line with `tabs`: closed ones fold away, the rest update, in order. */
  update(tabs: Tab[]): void {
    const motion = !this.deps.reducedMotion();
    const options = { holdOrder: this.deps.drag.state.id !== null, animateNew: this.drawn && motion, fold: motion };
    const folding = new Set(this.state.items.filter((item) => item.closing).map((item) => item.tab.id));
    const items = mergeItems(this.state.items, tabs, options);
    items.filter((item) => item.closing && !folding.has(item.tab.id)).forEach((item) => this.foldAway(item.tab.id));
    this.drawn = true;
    this.set({ tabs, items });
  }

  /** The view measured the room the tabs share. */
  setRoom(room: number): void {
    this.set({ room });
  }

  /** Opens a new tab. */
  newTab(): void {
    void this.deps.bridge.newTab();
  }

  /** Shows tab `id` (Enter or Space on its button). */
  activate(id: number): void {
    void this.deps.bridge.activateTab(id);
  }

  /** Closes tab `id` (Delete on it). */
  close(id: number): void {
    void this.deps.bridge.closeTab(id);
  }

  /** Closes a tab the mouse closed, holding every width so the next close button lands under the pointer. */
  closeByMouse(id: number): void {
    this.set({ frozen: this.state.frozen ?? tabWidth(this.state.room, this.state.tabs.length) });
    this.deps.card.hide();
    this.close(id);
  }

  /** The pointer left the strip: held widths let go and the tabs fill the room again. */
  release(): void {
    if (this.state.frozen === null || this.deps.drag.state.id !== null) return;
    this.set({ frozen: null });
  }

  /** Asks the main process for tab `id`'s native menu. */
  menu(id: number): void {
    this.deps.card.hide();
    void this.deps.bridge.showTabMenu(id);
  }

  /** A tab finished growing in. */
  opened(id: number): void {
    const items = this.state.items.map((item) => (item.tab.id === id ? { ...item, opening: false } : item));
    this.set({ items });
  }

  /** An arrow, Home or End on tab `id`: shows the tab it moves to and answers its id (null for another key). */
  step(key: string, id: number): number | null {
    if (!isRovingKey(key)) return null;
    const open = this.state.items.filter((item) => !item.closing);
    const index = open.findIndex((item) => item.tab.id === id);
    const next = open[rovingIndex(key, index, open.length)].tab.id;
    this.activate(next);
    return next;
  }

  /** The pointer came up on a pressed tab: a drag reorders the strip now and tells the main process. */
  endPress(pointerId: number): void {
    const drop = this.deps.drag.release(pointerId);
    if (!drop) return;
    this.set({ items: moveItem(this.state.items, drop.id, drop.to), drop });
    // Refused (an agent took control): the strip goes back to the main process's order.
    this.deps.bridge.moveTab(drop.id, drop.to).catch(() => this.update(this.state.tabs));
  }

  /** Takes a folded tab away once its fold has played. */
  private foldAway(id: number): void {
    const timer = setTimeout(() => {
      this.folds.delete(timer);
      this.set({ items: this.state.items.filter((item) => !(item.closing && item.tab.id === id)) });
    }, C.TAB_CLOSE_MS);
    this.folds.add(timer);
  }
}
