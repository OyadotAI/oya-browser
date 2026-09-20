/**
 * The TOON page renderer (toonformat.dev): a `page` object of facts, then one
 * table of blocks in reading order, each heading, paragraph, list item, table
 * row, image and interactive element a row. The columns are named once, so a
 * model reads the page, its element ids and their state in few tokens. Also the
 * small TOON writer the markdown renderer's element index uses.
 */
const { PAGE_RENDER } = require('../constants.cjs');

/** The block table's columns. */
const COLUMNS = ['id', 'region', 'kind', 'text', 'target', 'state'];

/** A cell that must be quoted: empty, padded, a keyword or number, or carrying TOON syntax. */
const NEEDS_QUOTES = /^$|^\s|\s$|^(true|false|null)$|^-?\d+(\.\d+)?(e[+-]?\d+)?$|[:,"\\[\]{}\n\r\t]|^-|^#$/i;

/** The line that opens a block table: its row count and columns. */
const BLOCKS_HEADER = /^blocks\[(\d+)\](\{[^}]*\}):$/;

/** One cell: numbers as they are, strings quoted (JSON style) only when TOON needs it. */
function toonCell(value) {
  if (typeof value === 'number' && Number.isFinite(value)) return String(value);
  const text = value == null ? '' : String(value);
  return NEEDS_QUOTES.test(text) ? JSON.stringify(text) : text;
}

/** A tabular array: `name[rows]{a,b}:` then one indented row per item. */
function toonTable(name, columns, rows) {
  const header = `${name}[${rows.length}]{${columns.join(',')}}:`;
  return [header, ...rows.map((row) => '  ' + columns.map((c) => toonCell(row[c])).join(','))].join('\n');
}

/** The page's facts as a TOON object, leaving out the ones that do not apply. */
function factsObject(facts = {}) {
  const lines = ['page:'];
  for (const [key, value] of Object.entries(facts)) {
    if (value !== undefined && value !== null && value !== '') lines.push(`  ${key}: ${toonCell(value)}`);
  }
  return lines;
}

/** The whole page as TOON. */
function render({ facts, blocks = [] }) {
  return [...factsObject(facts), toonTable('blocks', COLUMNS, blocks)].join('\n');
}

/** As many rows as fit in `room` characters. */
function rowsThatFit(rows, room) {
  const kept = [];
  for (const row of rows) {
    if (room < row.length + 1) break;
    kept.push(row);
    room -= row.length + 1;
  }
  return kept;
}

/** The facts, a truncated fact, then as many rows as fit, under a header that counts them. */
function cutRows(facts, header, rows, max) {
  const [, total, columns] = BLOCKS_HEADER.exec(header);
  const room = max - facts.join('\n').length - header.length - PAGE_RENDER.TRUNCATED_ROOM;
  const kept = rowsThatFit(rows, room);
  const note = `  truncated: "showing ${kept.length} of ${total} blocks; scroll and analyze again for the rest"`;
  const keptFacts = facts.filter((line) => !line.startsWith('  truncated: '));
  return [...keptFacts, note, `blocks[${kept.length}]${columns}:`, ...kept].join('\n');
}

/** The page within `max` characters: the facts stay whole, rows are cut at a row, and the TOON stays valid. */
function fit(text, max) {
  if (text.length <= max) return text;
  const lines = text.split('\n');
  const at = lines.findIndex((line) => BLOCKS_HEADER.test(line));
  if (at < 0) return text.slice(0, max);
  return cutRows(lines.slice(0, at), lines[at], lines.slice(at + 1), max);
}

/** What the agent reads: the page as TOON within its budget (every element is already a row). */
function forAgent(analysis, max) {
  return fit(render(analysis), max);
}

/** How the agent is told to read this format, for its system prompt. */
const guide = [
  '- analyze_page returns the page as TOON: a page object of facts, then blocks[N]{id,region,kind,text,target,state} with one row per heading, paragraph, list item, table row, image and interactive element, in reading order.',
  '- Page facts: url, title, scroll (how far down the document is), and when they apply: panelScroll (the content scrolls inside a panel, so scroll to see more even at 0%), modal (only that dialog was read), covered (something drawn over elements must be closed first), truncated (rows left out; scroll and analyze again).',
  '- A row with an id is an element you can act on by that id. kind says what it is (link, button, input:email, select, checkbox, radio, editable, textarea); text is its name; target is where a link goes or what a field holds now; state lists checked/unchecked, disabled, required, readonly, invalid with the page message, expanded/collapsed, selected, current, pressed, covered, off-screen, and hint (the format a field expects, or a select options).',
  '- Rows without an id are content: h1-h6 headings, text, item (list item), row and header (table rows, cells separated by |), quote, code, image (its alt text). region names the part of the page (nav, main, form/..., dialog).',
].join('\n');

module.exports = { format: 'toon', render, fit, forAgent, guide, toonCell, toonTable, COLUMNS };
