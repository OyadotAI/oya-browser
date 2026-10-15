/**
 * The one place a cut decides which blocks of a page to keep. A page too long
 * for the agent's budget used to keep its top, and on a scrolled page the top is
 * the nav bar, not what is on screen. An anchored cut (an on-screen element
 * exists) keeps, in priority order:
 *   T0  the on-screen elements, in reading order;
 *   T1  content rows (no id) within SCREEN_TEXT_REACH blocks of an on-screen element;
 *   T2  the off-screen form fields, wherever they are;
 *   T3  every other block, nearest first by its distance to the nearest on-screen element.
 * Every renderer's cut goes through here, so they agree on what the agent sees.
 */
import { CUT_TIER, PAGE_RENDER } from '../constants.ts';
import type { Block } from './types.ts';

/** The word the analyzer adds to an element's state when it is not on screen. */
const OFF_SCREEN = 'off-screen';

/** Element kinds a person fills in rather than clicks; every renderer shares this one set. */
export const FIELD_TYPES = new Set(['input', 'textarea', 'select', 'checkbox', 'radio', 'editable']);

/** What a cut kept: the kept blocks in reading order and in priority order, and whether an on-screen element anchored it. */
export interface Cut {
  /** The kept blocks' indices, in reading order. */
  kept: number[];
  /** The same blocks, most important first, so a cut that must drop more drops the least important first. */
  ranked: number[];
  /** Whether an on-screen element was found, so the cut is around the screen and not the top. */
  anchored: boolean;
}

/** The note a cut leaves in the page: how many blocks it kept, and where they were taken from. */
export const cutNote = (kept: number, total: number, anchored: boolean): string =>
  `showing ${kept} of ${total} blocks ${anchored ? 'around what is on screen' : 'from the top'}; scroll and analyze again for the rest`;

/** A block with an id: an element the agent can act on. */
const isElement = (block: Block): boolean => Boolean(block.id);

/** An element whose state does not carry the off-screen word. */
const onScreen = (block: Block): boolean => isElement(block) && !(block.state ?? '').split('; ').includes(OFF_SCREEN);

/** A form field: an element whose kind, before any ':', is a field type (input:email is an input). */
const isField = (block: Block): boolean => isElement(block) && FIELD_TYPES.has(block.kind.split(':')[0]);

/** Per index: how many blocks back the last `true` is (Infinity when there is none). */
function gapBack(flags: boolean[]): number[] {
  let last = -Infinity;
  return flags.map((flag, i) => {
    if (flag) last = i;
    return i - last;
  });
}

/** How far each block is from the nearest on-screen element, and which way. */
interface Distances {
  /** Blocks between it and the nearest on-screen element. */
  dist: number[];
  /** Whether that nearest element is above it, so it sits below the screen. */
  belowAnchor: boolean[];
}

/** Per block: its distance in blocks to the nearest on-screen element, and whether that element is above it. */
function distances(onScreenAt: boolean[]): Distances {
  const above = gapBack(onScreenAt);
  const below = gapBack([...onScreenAt].reverse()).reverse();
  return {
    dist: above.map((gap, i) => Math.min(gap, below[i])),
    belowAnchor: above.map((gap, i) => gap <= below[i]),
  };
}

/** Nearest first; on a tie the block below its anchor comes first, then the lower index. */
function nearestFirst(dist: number[], belowAnchor: boolean[]): (a: number, b: number) => number {
  return (a, b) => dist[a] - dist[b] || Number(belowAnchor[b]) - Number(belowAnchor[a]) || a - b;
}

/** The tier of a block in an anchored cut, from whether it is on screen and how far it is from the screen. */
function tierOf(block: Block, shown: boolean, dist: number): number {
  if (shown) return CUT_TIER.SCREEN;
  if (!isElement(block) && dist <= PAGE_RENDER.SCREEN_TEXT_REACH) return CUT_TIER.BESIDE;
  return isField(block) ? CUT_TIER.FIELD : CUT_TIER.REST;
}

/** The order once an on-screen element anchors the cut: by tier, then nearest first within a tier. */
function anchoredOrder(blocks: Block[]): number[] {
  const shown = blocks.map(onScreen);
  const { dist, belowAnchor } = distances(shown);
  const tier = blocks.map((block, i) => tierOf(block, shown[i], dist[i]));
  const near = nearestFirst(dist, belowAnchor);
  return blocks.map((_, i) => i).sort((a, b) => tier[a] - tier[b] || near(a, b));
}

/** The blocks in the order a cut takes them, and whether an on-screen element anchors that order. */
export interface Priority {
  /** The blocks' indices, most important first. */
  order: number[];
  /** Whether an on-screen element was found; when false the order is reading order from the top. */
  anchored: boolean;
}

/** The priority order of the blocks. Nothing on screen means plain reading order from the top. */
export function keepOrder(blocks: Block[]): Priority {
  const anchored = blocks.some(onScreen);
  const order = anchored ? anchoredOrder(blocks) : blocks.map((_, i) => i);
  return { order, anchored };
}

/** A test that takes a block when it fits what is left of `room`, and spends its size when it does. */
function budget(room: number, sizes: number[]): (i: number) => boolean {
  let left = room;
  return (i) => {
    if (sizes[i] > left) return false;
    left -= sizes[i];
    return true;
  };
}

/** The indices taken in `order` while each fits in what is left of `room`. */
// why: an anchored cut (skip) takes the next block that fits, so one oversize block cannot push out the on-screen part.
// why: an unanchored cut stops at the first miss, which keeps the top of the page exactly as it always was.
function takeInOrder(order: number[], sizes: number[], room: number, skip: boolean): number[] {
  const fits = budget(room, sizes);
  if (skip) return order.filter(fits);
  const miss = order.findIndex((i) => !fits(i));
  return miss < 0 ? order : order.slice(0, miss);
}

/** The blocks a cut keeps. `size` is a block's length in the format's text. */
export function pickBlocks(blocks: Block[], room: number, size: (block: Block) => number): Cut {
  const { order, anchored } = keepOrder(blocks);
  const sizes = blocks.map((block) => size(block));
  const ranked = takeInOrder(order, sizes, room, anchored);
  return { kept: [...ranked].sort((a, b) => a - b), ranked, anchored };
}
