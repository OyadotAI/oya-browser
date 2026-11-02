/**
 * The TOON page renderer (toonformat.dev): a `page` object of facts, then one
 * table of blocks in reading order, each heading, paragraph, list item, table
 * row, image and interactive element a row. The columns are named once, so a
 * model reads the page, its element ids and their state in few tokens. Also the
 * small TOON writer the markdown renderer's element index uses.
 */
import { PAGE_RENDER } from '../constants.ts';
import { GAP_ROW, cutNote, keepOrder, keptWithGaps, pickBlocks, refit, totalOf, type Priority } from './cut.ts';
import type { Analysis, Block, Facts } from './types.ts';

/** The format's name in configuration. */
export const format = 'toon';

/** The block table's columns. */
export const COLUMNS = ['id', 'region', 'kind', 'text', 'target', 'state'];

/** A cell that must be quoted: empty, padded, a keyword or number, or carrying TOON syntax. */
const NEEDS_QUOTES = /^$|^\s|\s$|^(true|false|null)$|^-?\d+(\.\d+)?(e[+-]?\d+)?$|[:,"\\[\]{}\n\r\t]|^-|^#$/i;

/** One cell: numbers as they are, strings quoted (JSON style) only when TOON needs it. */
export function toonCell(value: unknown): string {
  if (typeof value === 'number' && Number.isFinite(value)) return String(value);
  const text = value == null ? '' : String(value);
  return NEEDS_QUOTES.test(text) ? JSON.stringify(text) : text;
}

/** One row of a tabular array, indented as it sits under its header. */
function toonRow(row: object, columns: readonly string[]): string {
  return '  ' + columns.map((c) => toonCell((row as Record<string, unknown>)[c])).join(',');
}

/** A tabular array: `name[rows]{a,b}:` then one indented row per item. */
export function toonTable(name: string, columns: readonly string[], rows: readonly object[]): string {
  const header = `${name}[${rows.length}]{${columns.join(',')}}:`;
  return [header, ...rows.map((row) => toonRow(row, columns))].join('\n');
}

/** The block table's header line for `count` rows, as a cut measures it before it knows how many it keeps. */
const blocksHeader = (count: number): string => `blocks[${count}]{${COLUMNS.join(',')}}:`;

/** The rows `kept` names in reading order, with a gap row where rows were left out (when `gaps`). */
function tableRows(analysis: Analysis, blocks: Block[], kept: number[], gaps: boolean): Block[] {
  if (!gaps) return kept.map((i) => blocks[i]);
  return keptWithGaps(blocks, kept, totalOf(analysis)).map((i) => (i === null ? GAP_ROW : blocks[i]));
}

/** The page's facts as a TOON object, leaving out the ones that do not apply. */
function factsObject(facts: Facts = {}): string[] {
  const lines = ['page:'];
  for (const [key, value] of Object.entries(facts)) {
    if (value !== undefined && value !== null && value !== '') lines.push(`  ${key}: ${toonCell(value)}`);
  }
  return lines;
}

/** The whole page as TOON. */
export function render(analysis: Analysis, gaps = true): string {
  const blocks = analysis.blocks || [];
  const all = blocks.map((_, i) => i);
  return [...factsObject(analysis.facts), toonTable('blocks', COLUMNS, tableRows(analysis, blocks, all, gaps))].join(
    '\n',
  );
}

/** One page within `room`: the facts, the note that counts the blocks kept, and their rows. */
function pageFor(analysis: Analysis, room: number, gaps: boolean, priority: Priority): string {
  const blocks = analysis.blocks || [];
  const facts = factsObject(analysis.facts).filter((line) => !line.startsWith('  truncated: '));
  const picked = pickBlocks(blocks, room, (b) => toonRow(b, COLUMNS).length + 1, priority);
  const note = `  truncated: "${cutNote(picked.kept.length, totalOf(analysis), picked.anchored)}"`;
  const rows = tableRows(analysis, picked.blocks, picked.kept, gaps);
  return [...facts, note, toonTable('blocks', COLUMNS, rows)].join('\n');
}

/** The room a page leaves for its blocks: the budget, less the facts, the table header and the note. */
function blockRoom(analysis: Analysis, max: number): number {
  const facts = factsObject(analysis.facts).filter((line) => !line.startsWith('  truncated: '));
  const header = blocksHeader((analysis.blocks || []).length);
  return max - facts.join('\n').length - header.length - PAGE_RENDER.TRUNCATED_ROOM;
}

/**
 * The page within `max` characters for the agent. The facts stay whole; the blocks kept are the
 * ones cut.ts picks (the part on screen first), in reading order, under a count that says so.
 */
export function cut(analysis: Analysis, max: number, gaps = true): string {
  const whole = render(analysis, gaps);
  if (whole.length <= max) return whole;
  const priority = keepOrder(analysis.blocks || []);
  const pageAt = (room: number) => pageFor(analysis, room, gaps, priority);
  return refit(max, blockRoom(analysis, max), pageAt, () => pageAt(0));
}

/** The controls alone: the rows that carry an id, cut the same way, with none of the page's words. */
export function controls(analysis: Analysis, max: number): string {
  const acting = (analysis.blocks || []).filter((b: Block) => b.id);
  return cut({ facts: analysis.facts, blocks: acting }, max, false);
}

/** What the agent reads: the page as TOON within its budget (every element is already a row). */
export function forAgent(analysis: Analysis, max: number): string {
  return cut(analysis, max);
}

/** How the agent is told to read this format, for its system prompt. */
export const guide = [
  '- analyze_page returns the page as TOON: a page object of facts, then blocks[N]{id,region,kind,text,target,state} with one row per heading, paragraph, list item, table row, image and interactive element, in reading order.',
  '- Page facts: url, title, scroll (how far down the document is), and when they apply: panelScroll (the content scrolls inside a panel, so scroll to see more even at 0%), modal (only that dialog was read), covered (something drawn over elements must be closed first), truncated (rows left out; it says which part was kept, around what is on screen or from the top; scroll and analyze again for another part).',
  '- A row with an id is an element you can act on by that id. kind says what it is (link, button, input:email, select, checkbox, radio, editable, textarea); text is its name; target is where a link goes or what a field holds now; state lists checked/unchecked, disabled, required, readonly, invalid with the page message, expanded/collapsed, selected, current, pressed, covered, off-screen, and hint (the format a field expects, or a select options).',
  '- Rows without an id are content: h1-h6 headings, text, item (list item), row and header (table rows, cells separated by |), quote, code, image (its alt text). region names the part of the page (nav, main, form/..., dialog). A row of kind gap marks rows left out there; scroll and analyze again to see them.',
].join('\n');
