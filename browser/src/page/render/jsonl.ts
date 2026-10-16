/**
 * The JSONL page renderer: one JSON object per line. The first line is the
 * page's facts, `{"page":{…}}`; each line after it is one block in reading
 * order, a heading, paragraph, list item, table row, image or interactive
 * element, with its empty fields left out. Every line parses on its own, so a
 * program can stream it and a cut page stays valid.
 */
import { PAGE_RENDER } from '../constants.ts';
import { GAP_ROW, cutNote, keepOrder, keptWithGaps, pickBlocks, refit, totalOf, type Priority } from './cut.ts';
import { COLUMNS } from './toon.ts';
import type { Analysis, Block } from './types.ts';

/** The format's name in configuration. */
export const format = 'jsonl';

/** A block without its empty fields. */
function compact(block: object): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(block))
    if (value !== undefined && value !== null && value !== '') out[key] = value;
  return out;
}

/** The facts line. */
const factsLine = (facts: object = {}): string => JSON.stringify({ page: compact(facts) });

/** One block's line: the columns toon.ts names, without the empty fields, so the analyzer's screen and index stay out. */
const blockLine = (block: Block): string =>
  JSON.stringify(compact(Object.fromEntries(COLUMNS.map((column) => [column, block[column]]))));

/** The lines of the rows `kept` names in reading order, with a gap line where rows were left out (when `gaps`). */
function linesOf(analysis: Analysis, blocks: Block[], kept: number[], gaps: boolean): string[] {
  if (!gaps) return kept.map((i) => blockLine(blocks[i]));
  return keptWithGaps(blocks, kept, totalOf(analysis)).map((i) => blockLine(i === null ? GAP_ROW : blocks[i]));
}

/** The whole page as JSONL. */
export function render(analysis: Analysis, gaps = true): string {
  const blocks = analysis.blocks || [];
  const all = blocks.map((_, i) => i);
  return [factsLine(analysis.facts), ...linesOf(analysis, blocks, all, gaps)].join('\n');
}

/** One page within `room`: the facts line, the note that counts the blocks kept, and their lines. */
function pageFor(analysis: Analysis, room: number, gaps: boolean, priority: Priority): string {
  const picked = pickBlocks(analysis.blocks || [], room, (b) => blockLine(b).length + 1, priority);
  const facts = compact(analysis.facts ?? {});
  const page = { ...facts, truncated: cutNote(picked.kept.length, totalOf(analysis), picked.anchored) };
  return [JSON.stringify({ page }), ...linesOf(analysis, picked.blocks, picked.kept, gaps)].join('\n');
}

/** The room a page leaves for its blocks: the budget, less the facts line and the note. */
function blockRoom(analysis: Analysis, max: number): number {
  return max - JSON.stringify({ page: compact(analysis.facts ?? {}) }).length - PAGE_RENDER.TRUNCATED_ROOM;
}

/**
 * The page within `max` characters for the agent. The facts line stays whole; the lines kept are the
 * blocks cut.ts picks (the part on screen first), in reading order, with gap lines, and every line still parses.
 */
export function cut(analysis: Analysis, max: number, gaps = true): string {
  const whole = render(analysis, gaps);
  if (whole.length <= max) return whole;
  const priority = keepOrder(analysis.blocks || []);
  const pageAt = (room: number) => pageFor(analysis, room, gaps, priority);
  return refit(max, blockRoom(analysis, max), pageAt, () => pageAt(0));
}

/** The controls alone: the lines that carry an id, cut the same way, with none of the page's words. */
export function controls(analysis: Analysis, max: number): string {
  const acting = (analysis.blocks || []).filter((b: Block) => b.id);
  return cut({ facts: analysis.facts, blocks: acting }, max, false);
}

/** What the agent reads: the page as JSONL within its budget (every element is already a line). */
export function forAgent(analysis: Analysis, max: number): string {
  return cut(analysis, max);
}

/** How the agent is told to read this format, for its system prompt. */
export const guide = [
  '- analyze_page returns the page as JSONL: the first line is {"page":{…facts}}, then one JSON object per heading, paragraph, list item, table row, image and interactive element, in reading order, with fields id, region, kind, text, target, state (empty ones left out).',
  '- Page facts: url, title, scroll, and when they apply: panelScroll (the content scrolls inside a panel), modal (only that dialog was read), covered (something drawn over elements must be closed first), truncated (lines left out; it says which part was kept, around what is on screen or from the top; scroll and analyze again for another part).',
  '- A line with an id is an element you can act on by that id. kind says what it is (link, button, input:email, select, checkbox, radio, editable, textarea); text is its name; target is where a link goes or what a field holds now; state lists checked/unchecked, disabled, required, readonly, invalid with the page message, expanded/collapsed, selected, current, pressed, covered, off-screen, and hint (the format a field expects, or a select options).',
  '- Lines without an id are content: h1-h6 headings, text, item (list item), row and header (table rows, cells separated by |), quote, code, image (its alt text). region names the part of the page (nav, main, form/..., dialog). A line of kind gap marks lines left out there; scroll and analyze again to see them.',
].join('\n');
