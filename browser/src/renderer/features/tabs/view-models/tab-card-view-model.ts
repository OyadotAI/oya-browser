/**
 * The tab hover card, like Chrome's: rest the pointer on a tab and its full
 * title and address show under it, instead of the native tooltip. Once one
 * card is up, moving along the strip shows the next at once. The strip and
 * a drag hide it, so it is a ViewModel they share.
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { Tab } from '../model/tab-model.ts';

/** What the card shows. */
export interface TabCardState {
  /** The tab on the card, or null while it is hidden. */
  tab: Tab | null;
  /** The left edge of that tab, in window pixels. */
  left: number;
}

/** The hover card. */
export class TabCardViewModel extends ViewModel<TabCardState> {
  /** The timer that shows the card after the pointer rests. */
  private timer: ReturnType<typeof setTimeout> | undefined;

  /** Hidden; a pending show is cancelled on dispose. */
  constructor() {
    super({ tab: null, left: 0 });
    this.own(() => clearTimeout(this.timer));
  }

  /** The pointer came onto `tab`, whose left edge is at `left`: its card shows now if one is up, else after a rest. */
  hover(tab: Tab, left: number): void {
    clearTimeout(this.timer);
    if (this.state.tab) return this.set({ tab, left });
    this.timer = setTimeout(() => this.set({ tab, left }), C.TAB_CARD_DELAY_MS);
  }

  /** Hides the card and forgets any card waiting to show. */
  hide(): void {
    clearTimeout(this.timer);
    this.set({ tab: null });
  }
}
