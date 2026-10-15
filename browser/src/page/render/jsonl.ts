/**
 * The JSONL page renderer: one JSON object per line. The first line is the
 * page's facts, `{"page":{…}}`; each line after it is one block in reading
 * order, a heading, paragraph, list item, table row, image or interactive
 * element, with its empty fields left out. Every line parses on its own, so a
 * program can stream it and a cut page stays valid.
 */
import { PAGE_RENDER } from '../constants.ts';
import { cutNote, pickBlocks } from './cut.ts';
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

/** One block's line: its JSON without the empty fields. */
const blockLine = (block: object): string => JSON.stringify(compact(block));

/** The whole page as JSONL. */
export function render({ facts, blocks = [] }: Analysis): string {
  return [factsLine(facts), ...blocks.map(blockLine)].join('\n');
}

/** As many lines as fit in `room` characters. */
function linesThatFit(lines: string[], room: number): string[] {
  const kept: string[] = [];
  for (const line of lines) {
    if (room < line.length + 1) break;
    kept.push(line);
    room -= line.length + 1;
  }
  return kept;
}

/** The page within `max` characters, cut at a line from the top, the facts line saying how many blocks were kept. */
export function fit(text: string, max: number): string {
  if (text.length <= max) return text;
  const [first, ...lines] = text.split('\n');
  const { page } = JSON.parse(first);
  const kept = linesThatFit(lines, max - first.length - PAGE_RENDER.TRUNCATED_ROOM);
  const truncated = cutNote(kept.length, lines.length, false);
  return [JSON.stringify({ page: { ...page, truncated } }), ...kept].join('\n');
}

/**
 * The page within `max` characters for the agent. The facts line stays whole; the lines kept are the
 * blocks cut.ts picks (the part on screen first), in reading order, and every line still parses.
 */
export function cut(analysis: Analysis, max: number): string {
  const whole = render(analysis);
  if (whole.length <= max) return whole;
  const blocks = analysis.blocks || [];
  const facts = compact(analysis.facts ?? {});
  const room = max - JSON.stringify({ page: facts }).length - PAGE_RENDER.TRUNCATED_ROOM;
  const { kept, anchored } = pickBlocks(blocks, room, (b) => blockLine(b).length + 1);
  const page = { ...facts, truncated: cutNote(kept.length, analysis.totalBlocks ?? blocks.length, anchored) };
  return [JSON.stringify({ page }), ...kept.map((i) => blockLine(blocks[i]))].join('\n');
}

/** The controls alone: the lines that carry an id, cut the same way, with none of the page's words. */
export function controls(analysis: Analysis, max: number): string {
  const acting = (analysis.blocks || []).filter((b: Block) => b.id);
  return cut({ facts: analysis.facts, blocks: acting }, max);
}

/** What the agent reads: the page as JSONL within its budget (every element is already a line). */
export function forAgent(analysis: Analysis, max: number): string {
  return cut(analysis, max);
}

/** How the agent is told to read this format, for its system prompt. */
export const guide = [
  '- analyze_page returns the page as JSONL: the first line is {"page":{…facts}}, then one JSON object per heading, paragraph, list item, table row, image and interactive element, in reading order, with fields id, region, kind, text, target, state (empty ones left out).',
  '- Page facts: url, title, scroll, and when they apply: panelScroll (the content scrolls inside a panel), modal (only that dialog was read), covered (something drawn over elements must be closed first), truncated (lines left out: a long page keeps the part on screen, the form fields and the lines nearest them; scroll and analyze again for another part).',
  '- A line with an id is an element you can act on by that id. kind says what it is (link, button, input:email, select, checkbox, radio, editable, textarea); text is its name; target is where a link goes or what a field holds now; state lists checked/unchecked, disabled, required, readonly, invalid with the page message, expanded/collapsed, selected, current, pressed, covered, off-screen, and hint (the format a field expects, or a select options).',
  '- Lines without an id are content: h1-h6 headings, text, item (list item), row and header (table rows, cells separated by |), quote, code, image (its alt text). region names the part of the page (nav, main, form/..., dialog).',
].join('\n');
