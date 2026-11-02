/**
 * The tab strip's arithmetic, with no DOM: how wide tabs are, which size
 * they draw at, where a dragged tab lands, how far its neighbours slide out
 * of its way, how fast the strip scrolls under it, and where the hover card
 * sits. Pure, so it is tested on its own.
 */
import { RendererConstants as C } from '../../../core/constants.ts';

/** Where one tab sits on the strip, in the strip's own pixels. */
export interface Slot {
  /** Its left edge. */
  left: number;
  /** Its width. */
  width: number;
}

/** The size a tab draws at: 'small' drops an inactive tab's close button, 'tiny' shows only its icon. */
export type TabSize = 'normal' | 'small' | 'tiny';

/** How many tabs lead the strip pinned (the main process keeps them first). */
export const pinnedCount = (tabs: readonly { /** Pinned to the leading block. */ pinned?: boolean }[]): number =>
  tabs.filter((t) => t.pinned).length;

/** Each tab's width: an even share of `available`, at most the widest a tab grows and at least the narrowest. */
export function tabWidth(available: number, count: number): number {
  if (count < 1) return C.TAB_MAX_WIDTH;
  return Math.max(C.TAB_MIN_WIDTH, Math.min(C.TAB_MAX_WIDTH, Math.floor(available / count)));
}

/** The size a tab of `width` draws at. */
export function tabSize(width: number): TabSize {
  if (width < C.TAB_TINY_WIDTH) return 'tiny';
  return width < C.TAB_SMALL_WIDTH ? 'small' : 'normal';
}

/** The middle of a slot. */
export const middle = (slot: Slot): number => slot.left + slot.width * C.HALF;

/** The right edge of a slot. */
const right = (slot: Slot): number => slot.left + slot.width;

/** The first and last slot a dragged tab may reach: its own group's, so pinned and unpinned tabs never cross. */
export interface SlotRange {
  /** The first slot it may reach. */
  first: number;
  /** The last slot it may reach. */
  last: number;
}

/** The slots a tab may be dragged across: the leading `pinned` slots for a pinned tab, the rest for the others. */
export function groupRange(pinned: number, count: number, isPinned: boolean): SlotRange {
  return isPinned ? { first: 0, last: pinned - 1 } : { first: pinned, last: count - 1 };
}

/** How far the tab in slot `from` may be dragged by `dx` without leaving the slots in `range` (the whole strip by default). */
export function clamp(slots: readonly Slot[], from: number, dx: number, range?: SlotRange): number {
  const { first, last } = range ?? { first: 0, last: slots.length - 1 };
  const least = slots[first].left - slots[from].left;
  const most = right(slots[last]) - right(slots[from]);
  return Math.max(least, Math.min(most, dx));
}

/** The index the tab in slot `from` lands at when dragged by `dx`: its leading edge passed the middle of each tab it displaces. */
export function dropIndex(slots: readonly Slot[], from: number, dx: number): number {
  const left = slots[from].left + dx;
  const end = left + slots[from].width;
  const passedRight = slots.filter((slot, i) => i > from && end > middle(slot)).length;
  const passedLeft = slots.filter((slot, i) => i < from && left < middle(slot)).length;
  return from + passedRight - passedLeft;
}

/** How far each tab slides so the one moving from `from` to `to` has room: its width, left or right, or 0. */
export function shifts(slots: readonly Slot[], from: number, to: number): number[] {
  const step = slots[from].width;
  return slots.map((_slot, i) => {
    if (from < to && i > from && i <= to) return -step;
    if (to < from && i >= to && i < from) return step;
    return 0;
  });
}

/** Where the tab from slot `from` sits once it has moved to `to`, the others closing up behind it. */
export function landingLeft(slots: readonly Slot[], from: number, to: number): number {
  return from < to ? right(slots[to]) - slots[from].width : slots[to].left;
}

/** How deep, 0 to 1, a pointer `distance` from an edge is in that edge's scroll zone. */
const into = (distance: number): number => Math.min(1, Math.max(0, (C.TAB_EDGE_PX - distance) / C.TAB_EDGE_PX));

/** Pixels a frame to scroll the strip for a pointer at `x` over a strip from `left` to `right`; 0 away from its edges. */
export function edgeSpeed(x: number, left: number, end: number): number {
  return Math.round((into(end - x) - into(x - left)) * C.TAB_SCROLL_MAX_PX);
}

/** The hover card's left edge: under its tab at `left`, inside a window `viewport` wide. */
export function cardLeft(left: number, viewport: number, cardWidth: number): number {
  return Math.max(0, Math.min(left, viewport - cardWidth));
}

/** Where an incoming tab sits in the strip: its index among the open tabs, its slot's left edge, and where it is drawn. */
export interface IncomingSlot {
  /** Its index among the open tabs. */
  to: number;
  /** Its slot's left edge. */
  slot: number;
  /** Where it is drawn, centred on the pointer but kept within the unpinned tabs. */
  left: number;
}

/**
 * Where a tab carried over the strip goes: `x` is the pointer in the strip's own
 * pixels, `start` where the unpinned tabs begin, `count` how many there are and
 * `width` each one's. It is drawn centred on the pointer, and its gap opens at the
 * slot it covers most, so the gap is always under the tab.
 */
export function incomingSlot(x: number, start: number, pinned: number, count: number, width: number): IncomingSlot {
  const at = Math.max(0, Math.min(count, Math.round((x - start) / width - C.HALF)));
  const left = Math.max(start, Math.min(start + count * width, x - width * C.HALF));
  return { to: pinned + at, slot: start + at * width, left };
}
