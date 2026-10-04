/**
 * The tab strip's arithmetic, with no DOM: how wide tabs are, which size
 * they draw at, where a dragged tab lands, how far its neighbours slide out
 * of its way, and how fast the strip scrolls under it. Pure, so it is tested
 * on its own.
 */
/* global RendererConstants */
/* exported TabMath */

/** The strip's arithmetic. */
const TabMath = {
  /** Each tab's width: an even share of `available`, at most the widest a tab grows and at least the narrowest. */
  width(available, count) {
    const { TAB_MAX_WIDTH, TAB_MIN_WIDTH } = RendererConstants;
    if (count < 1) return TAB_MAX_WIDTH;
    return Math.max(TAB_MIN_WIDTH, Math.min(TAB_MAX_WIDTH, Math.floor(available / count)));
  },

  /** The size a tab of `width` draws at: 'normal', 'small' (no close on inactive tabs) or 'tiny' (icon only). */
  size(width) {
    if (width < RendererConstants.TAB_TINY_WIDTH) return 'tiny';
    return width < RendererConstants.TAB_SMALL_WIDTH ? 'small' : 'normal';
  },

  /** The middle of a slot (`{ left, width }`). */
  middle: (slot) => slot.left + slot.width * RendererConstants.HALF,

  /** How far the tab in slot `from` may be dragged by `dx` without leaving the strip's first and last slots. */
  clamp(slots, from, dx) {
    const first = slots[0].left - slots[from].left;
    const last = slots.at(-1).left + slots.at(-1).width - (slots[from].left + slots[from].width);
    return Math.max(first, Math.min(last, dx));
  },

  /** The index the tab in slot `from` lands at when dragged by `dx`: its leading edge passed the middle of each tab it displaces. */
  dropIndex(slots, from, dx) {
    const left = slots[from].left + dx;
    const right = left + slots[from].width;
    const passedRight = slots.filter((slot, i) => i > from && right > TabMath.middle(slot)).length;
    const passedLeft = slots.filter((slot, i) => i < from && left < TabMath.middle(slot)).length;
    return from + passedRight - passedLeft;
  },

  /** How far each tab slides so the one moving from `from` to `to` has room: its width, left or right, or 0. */
  shifts(slots, from, to) {
    const step = slots[from].width;
    return slots.map((_slot, i) => {
      if (from < to && i > from && i <= to) return -step;
      if (to < from && i >= to && i < from) return step;
      return 0;
    });
  },

  /** Pixels a frame to scroll the strip for a pointer at `x` over a strip from `left` to `right`; 0 away from its edges. */
  edgeSpeed(x, left, right) {
    const { TAB_EDGE_PX, TAB_SCROLL_MAX_PX } = RendererConstants;
    const into = (distance) => Math.min(1, Math.max(0, (TAB_EDGE_PX - distance) / TAB_EDGE_PX));
    return Math.round((into(right - x) - into(x - left)) * TAB_SCROLL_MAX_PX);
  },
};
