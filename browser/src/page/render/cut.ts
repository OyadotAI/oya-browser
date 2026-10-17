/**
 * The one place a cut decides which blocks of a long page to keep. A page too
 * long for the agent's budget used to keep its top, and on a scrolled page the
 * top is the nav bar, not what is on screen. The analyzer marks each row that is
 * on screen (`screen: true`). When any row is marked, the cut keeps the rows
 * nearest the screen first, measured in page positions (`index`); when none is
 * marked, it keeps the top in reading order. Every renderer's cut goes through
 * here, so they agree on what the agent sees.
 */
import { PAGE_RENDER } from '../constants.ts';
import type { Analysis, Block } from './types.ts';

/** What a cut kept: the kept blocks in reading order, whether an on-screen row anchored it, and the blocks to render. */
export interface Cut {
  /** The kept blocks' indices, in reading order; they index `blocks`. */
  kept: number[];
  /** Whether an on-screen row was found, so the cut is around the screen and not the top. */
  anchored: boolean;
  /** The page's blocks as a renderer writes them: the same positions, with a long text clipped to the room. */
  blocks: Block[];
}

/** The note a cut leaves in the page: how many blocks it kept, and where they were taken from. */
export const cutNote = (kept: number, total: number, anchored: boolean): string =>
  `showing ${kept} of ${total} blocks ${anchored ? 'around what is on screen' : 'from the top'}; scroll and analyze again for the rest`;

/** How many blocks the analyzer read in all: its own count before its cap, or the blocks it sent when it sent no count. */
export const totalOf = (analysis: Analysis): number => analysis.totalBlocks ?? (analysis.blocks || []).length;

/** A row the analyzer marked on screen. The only anchor: page text cannot claim to be on screen by its words. */
// An older analyzer sends no screen mark; its state words are page text a page can forge, so such a page is cut from the top, as released builds did.
const isAnchor = (block: Block): boolean => block.screen === true;

/** Per block: the distance back to the last anchor at or before it, in page positions (Infinity when there is none). */
function backGap(at: number[], anchors: boolean[]): number[] {
  let last = -Infinity;
  return anchors.map((anchor, i) => {
    if (anchor) last = at[i];
    return at[i] - last;
  });
}

/** Per block: the distance forward to the next anchor at or after it, in page positions (Infinity when there is none). */
function forwardGap(at: number[], anchors: boolean[]): number[] {
  let next = Infinity;
  const gaps: number[] = [];
  for (let i = at.length - 1; i >= 0; i--) {
    if (anchors[i]) next = at[i];
    gaps[i] = next - at[i];
  }
  return gaps;
}

/** How far each block is from the nearest anchor. */
interface Distances {
  /** Per block: its distance in page positions to the nearest anchor. */
  dist: number[];
  /** Per block: whether that anchor is above it, so the block sits below it. */
  belowAnchor: boolean[];
}

/** Per block: its distance in page positions to the nearest anchor, and whether that anchor is above it. */
function anchorDistances(blocks: Block[]): Distances {
  const at = blocks.map((block, i) => block.index ?? i);
  const anchors = blocks.map(isAnchor);
  const back = backGap(at, anchors);
  const forward = forwardGap(at, anchors);
  return {
    dist: back.map((gap, i) => Math.min(gap, forward[i])),
    belowAnchor: back.map((gap, i) => gap <= forward[i]),
  };
}

/** Nearest the screen first; on a tie the block below its anchor comes first, then reading order. */
function anchoredOrder(blocks: Block[]): number[] {
  const { dist, belowAnchor } = anchorDistances(blocks);
  return blocks
    .map((_, i) => i)
    .sort((a, b) => dist[a] - dist[b] || Number(belowAnchor[b]) - Number(belowAnchor[a]) || a - b);
}

/** The blocks in the order a cut takes them, and whether an on-screen row anchors that order. */
export interface Priority {
  /** The blocks' indices, most important first. */
  order: number[];
  /** Whether an on-screen row was found; when false the order is reading order from the top. */
  anchored: boolean;
}

/** The priority order of the blocks. Nothing on screen means plain reading order from the top. */
export function keepOrder(blocks: Block[]): Priority {
  const anchored = blocks.some(isAnchor);
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

/**
 * The indices taken in `order` that fit in what is left of `room`. Around the screen a block that does not fit
 * is skipped and the walk goes on, so one big block near the screen cannot leave the rest of the room unused.
 * From the top the first miss stops the walk, so an unanchored cut stays one unbroken run, as before.
 */
function takeInOrder(order: number[], sizes: number[], room: number, skip: boolean): number[] {
  const fits = budget(room, sizes);
  if (skip) return order.filter(fits);
  const miss = order.findIndex((i) => !fits(i));
  return miss < 0 ? order : order.slice(0, miss);
}

/** A row that stands for rows left out between two shown ones: every column empty but the kind. */
export const GAP_ROW: Block = { kind: 'gap' };

/** The longest run of whole characters of `text` that is at most `max` UTF-16 units, so a surrogate pair is never split. */
function firstUnits(text: string, max: number): string {
  let out = '';
  for (const ch of text) {
    if (out.length + ch.length > max) break;
    out += ch;
  }
  return out;
}

/** A text clipped to `cap` units, ending in '…' when it was clipped. */
const clipText = (text: string, cap: number): string =>
  text.length > cap ? firstUnits(text, Math.max(cap - 1, 0)) + '…' : text;

/** A text shrunk in proportion to how far its size is over `cap`. */
const shrinkText = (text: string, cap: number, size: number): string =>
  clipText(text, Math.floor((text.length * cap) / size));

/** A block with its text replaced, and nothing else changed. */
const withText = (block: Block, text: string): Block => ({ ...block, text });

/** Measures a block's size in the format's text. */
type SizeOf = (block: Block) => number;

/** A block as it will be shown, with its size. */
interface Fitted {
  /** The block, with its text as it will be shown. */
  block: Block;
  /** Its size in the format's text. */
  size: number;
}

/** Every block as it will be shown, with each one's size. */
interface Showing {
  /** The blocks as they will be shown. */
  shown: Block[];
  /** Each block's size, in the same order. */
  sizes: number[];
}

/** A block shrunk in proportion while its size is over `cap`, at most MAX_SHRINK_PASSES times. */
function shrinkBlock(block: Block, cap: number, sizeOf: SizeOf): Fitted {
  let shown = block;
  let size = sizeOf(shown);
  for (let pass = 0; size > cap && pass < PAGE_RENDER.MAX_SHRINK_PASSES; pass++) {
    shown = withText(shown, shrinkText(shown.text as string, cap, size));
    size = sizeOf(shown);
  }
  return { block: shown, size };
}

/**
 * A block as it will be shown: its text clipped to `cap` and then shrunk while its size is over `cap` (escapes and
 * a format's markup can make a block bigger than its text). A text that still does not fit is dropped.
 */
function fitBlock(block: Block, cap: number, sizeOf: SizeOf): Fitted {
  if (typeof block.text !== 'string') return { block, size: sizeOf(block) };
  const shrunk = shrinkBlock(withText(block, clipText(block.text, cap)), cap, sizeOf);
  if (shrunk.size <= cap) return shrunk;
  const dropped = withText(shrunk.block, '');
  return { block: dropped, size: sizeOf(dropped) };
}

/** A block's length in the format's text, given the block and the block before it in the page (none for the first). */
export type Sizer = (block: Block, previous: Block | undefined) => number;

/** Every block as it will be shown, with its size. A size may depend on the shown block before it. */
function showAll(blocks: Block[], cap: number, size: Sizer): Showing {
  const shown: Block[] = [];
  const sizes: number[] = [];
  blocks.forEach((block, at) => {
    const fitted = fitBlock(block, cap, (b) => size(b, shown[at - 1]));
    shown.push(fitted.block);
    sizes.push(fitted.size);
  });
  return { shown, sizes };
}

/**
 * The blocks a cut keeps, and the blocks it renders. A block's text is clipped to its share of the room first,
 * so one huge block shows its start and leaves room for what is around it. `priority` is keepOrder(blocks); a
 * cut that picks the same page again passes it in, so the order is worked out once.
 */
export function pickBlocks(blocks: Block[], room: number, size: Sizer, priority = keepOrder(blocks)): Cut {
  const { shown, sizes } = showAll(blocks, Math.floor(room * PAGE_RENDER.MAX_BLOCK_SHARE), size);
  const kept = [...takeInOrder(priority.order, sizes, room, priority.anchored)].sort((a, b) => a - b);
  return { kept, anchored: priority.anchored, blocks: shown };
}

/**
 * The kept positions in reading order, with `null` wherever the analyzer's index says blocks were left out:
 * between two kept blocks, before the first, or after the last of the `total` blocks in the page.
 */
export function keptWithGaps(blocks: Block[], kept: number[], total: number): (number | null)[] {
  const out: (number | null)[] = [];
  let previous = -1;
  for (const i of kept) {
    const at = blocks[i].index ?? i;
    out.push(...(at === previous + 1 ? [] : [null]), i);
    previous = at;
  }
  return previous < total - 1 ? [...out, null] : out;
}

/**
 * Renders a page with `room` for its blocks and, while it is over `max`, renders it again with less room: the
 * overflow comes off the room twice, once for the overflow and once as slack for lines a pick may add. At most
 * MAX_REFITS times; a page still over `max` then comes from `fallback`.
 */
export function refit(max: number, room: number, render: (room: number) => string, fallback: () => string): string {
  let left = room;
  let page = render(left);
  for (let pass = 0; page.length > max && pass < PAGE_RENDER.MAX_REFITS; pass++) {
    const overflow = page.length - max;
    // The overflow comes off twice, since the lines a pick adds (gaps, region comments) are not in its size.
    left -= overflow + overflow;
    page = render(left);
  }
  return page.length <= max ? page : fallback();
}
