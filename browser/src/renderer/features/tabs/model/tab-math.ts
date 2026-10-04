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

/** How far the tab in slot `from` may be dragged by `dx` without leaving the strip's first and last slots. */
export function clamp(slots: readonly Slot[], from: number, dx: number): number {
  const first = slots[0].left - slots[from].left;
  const last = right(slots[slots.length - 1]) - right(slots[from]);
  return Math.max(first, Math.min(last, dx));
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
