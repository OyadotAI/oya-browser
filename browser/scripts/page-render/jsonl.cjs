/**
 * The JSONL page renderer: one JSON object per line. The first line is the
 * page's facts, `{"page":{…}}`; each line after it is one block in reading
 * order, a heading, paragraph, list item, table row, image or interactive
 * element, with its empty fields left out. Every line parses on its own, so a
 * program can stream it and a cut page stays valid.
 */
const { PAGE_RENDER } = require('../constants.cjs');

/** A block without its empty fields. */
function compact(block) {
  const out = {};
  for (const [key, value] of Object.entries(block))
    if (value !== undefined && value !== null && value !== '') out[key] = value;
  return out;
}

/** The facts line. */
const factsLine = (facts = {}) => JSON.stringify({ page: compact(facts) });

/** The whole page as JSONL. */
function render({ facts, blocks = [] }) {
  return [factsLine(facts), ...blocks.map((b) => JSON.stringify(compact(b)))].join('\n');
}

/** As many lines as fit in `room` characters. */
function linesThatFit(lines, room) {
  const kept = [];
  for (const line of lines) {
    if (room < line.length + 1) break;
    kept.push(line);
    room -= line.length + 1;
  }
  return kept;
}

/** The page within `max` characters, cut at a line, the facts line saying how many blocks were kept. */
function fit(text, max) {
  if (text.length <= max) return text;
  const [first, ...lines] = text.split('\n');
  const { page } = JSON.parse(first);
  const kept = linesThatFit(lines, max - first.length - PAGE_RENDER.TRUNCATED_ROOM);
  const truncated = `showing ${kept.length} of ${lines.length} blocks; scroll and analyze again for the rest`;
  return [JSON.stringify({ page: { ...page, truncated } }), ...kept].join('\n');
}

/** The controls alone: the lines that carry an id, with none of the page's words. */
function controls(analysis, max) {
  const acting = (analysis.blocks || []).filter((b) => b.id);
  return fit(render({ facts: analysis.facts, blocks: acting }), max);
}

/** What the agent reads: the page as JSONL within its budget (every element is already a line). */
function forAgent(analysis, max) {
  return fit(render(analysis), max);
}

/** How the agent is told to read this format, for its system prompt. */
const guide = [
  '- analyze_page returns the page as JSONL: the first line is {"page":{…facts}}, then one JSON object per heading, paragraph, list item, table row, image and interactive element, in reading order, with fields id, region, kind, text, target, state (empty ones left out).',
  '- Page facts: url, title, scroll, and when they apply: panelScroll (the content scrolls inside a panel), modal (only that dialog was read), covered (something drawn over elements must be closed first), truncated (lines left out; scroll and analyze again).',
  '- A line with an id is an element you can act on by that id. kind says what it is (link, button, input:email, select, checkbox, radio, editable, textarea); text is its name; target is where a link goes or what a field holds now; state lists checked/unchecked, disabled, required, readonly, invalid with the page message, expanded/collapsed, selected, current, pressed, covered, off-screen, and hint (the format a field expects, or a select options).',
  '- Lines without an id are content: h1-h6 headings, text, item (list item), row and header (table rows, cells separated by |), quote, code, image (its alt text). region names the part of the page (nav, main, form/..., dialog).',
].join('\n');

module.exports = { format: 'jsonl', render, fit, forAgent, controls, guide };
