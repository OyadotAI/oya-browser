/**
 * The markdown page renderer: the page's facts as a front-matter header, then
 * its blocks as markdown (headings, paragraphs, lists, tables, code) with each
 * interactive element as a tag, `[#12 link "Sign in" → /login]`. What the agent
 * reads also carries the element index, TOON tables of the form fields (label,
 * value, expected format, state), the other visible elements and the off-screen
 * ones, which the agent fills forms from.
 */
const { PAGE_RENDER } = require('../constants.cjs');
const { toonTable } = require('./toon.cjs');

/** Said where the page's markdown was cut to leave room for the index. */
const CUT = '\n\n⚠ Output truncated to fit context window.';

/** Element kinds a person fills in rather than clicks. */
const FIELD_TYPES = new Set(['input', 'textarea', 'select', 'checkbox', 'radio', 'editable']);

/** The field table's and the visible-element table's columns. */
const FIELD_COLUMNS = ['id', 'type', 'label', 'value', 'hint', 'state'];
const VISIBLE_COLUMNS = ['id', 'type', 'label', 'link'];

/** What the field table is, said once above it. */
const FIELDS_NOTE = 'Form fields (hint is the format the field expects, or a select’s options):\n';

/** Said after the index when the page was cut short. */
const TRUNCATED = '\n⚠ Page content was truncated (very long page). Scroll down and re-analyze to see more.\n';

/** How each content kind is written; element rows and table rows are written apart. */
const KIND_LINES = {
  text: (b) => b.text,
  item: (b) => `- ${b.text}`,
  quote: (b) => `> ${b.text}`,
  code: (b) => '```\n' + b.text + '\n```',
  image: (b) => `[image: ${b.text}]`,
  caption: (b) => `*${b.text}*`,
  term: (b) => `**${b.text}**`,
  iframe: (b) => `[iframe: ${b.target || ''}]`,
};

/** Kinds written one after another without a blank line between them. */
const RUNS = new Set(['item', 'row', 'header', 'element']);

/** An element as a tag: its id, kind, name, where it goes or what it holds, and its state. */
function elementTag(b) {
  const name = b.text ? ` "${String(b.text).replace(/"/g, '\\"')}"` : '';
  const target = b.target
    ? b.kind === 'link'
      ? ` → ${b.target}`
      : ` = "${String(b.target).replace(/"/g, '\\"')}"`
    : '';
  return `[#${b.id} ${b.kind}${name}${target}${b.state ? ` (${b.state})` : ''}]`;
}

/** One block as its markdown line, and the kind of run it belongs to. */
function blockLine(b) {
  if (b.id) return { line: elementTag(b), run: 'element' };
  const heading = /^h([1-6])$/.exec(b.kind);
  if (heading) return { line: `${'#'.repeat(Number(heading[1]))} ${b.text}`, run: 'heading' };
  if (b.kind === 'row' || b.kind === 'header') return { line: `| ${b.text} |`, run: b.kind };
  return { line: Object.hasOwn(KIND_LINES, b.kind) ? KIND_LINES[b.kind](b) : b.text, run: b.kind };
}

/** A table header's separator row, one --- per cell. */
const separator = (header) =>
  '| ' +
  header
    .split(' | ')
    .map(() => '---')
    .join(' | ') +
  ' |';

/** Whether a line starts a new paragraph: anything but the next item, row or element of a run. */
const breaksRun = (run, previous) => !(run === previous && RUNS.has(run)) && !(run === 'row' && previous === 'header');

/** Whether a row opens a table: a header row, or a first row (markdown needs the separator after it to see a table). */
const startsTable = (run, previous) =>
  run === 'header' || (run === 'row' && previous !== 'row' && previous !== 'header');

/** Appends one block's lines: a region comment when the region changes, a blank line between runs. */
function appendBlock(out, b, last) {
  if (b.region !== last.region) out.push('', `<!-- ${b.region || 'page'} -->`);
  const { line, run } = blockLine(b);
  if (breaksRun(run, last.run)) out.push('');
  out.push(line);
  if (startsTable(run, last.run)) out.push(separator(b.text));
  return { region: b.region, run };
}

/** The blocks as markdown: regions as comments, runs of items, rows and elements kept together. */
function body(blocks = []) {
  const out = [];
  let last = { region: null, run: null };
  for (const b of blocks) last = appendBlock(out, b, last);
  return out
    .join('\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

/** The facts as a front-matter header. */
function header(facts = {}) {
  const lines = Object.entries(facts)
    .filter(([, value]) => value !== undefined && value !== null && value !== '')
    .map(([key, value]) => `${key}: ${value}`);
  return `---\n${lines.join('\n')}\n---`;
}

/** The whole page as markdown. */
function render({ facts, blocks }) {
  return `${header(facts)}\n\n${body(blocks)}`;
}

/** The markdown cut at a line end to fit `max`, marked where it was cut. */
function fit(text, max) {
  if (text.length <= max) return text;
  const cut = text.slice(0, Math.max(max - CUT.length, 0));
  const end = cut.lastIndexOf('\n');
  return (end > 0 ? cut.slice(0, end) : cut) + CUT;
}

/** A checkbox or radio, whose value is its state rather than text. */
const isToggle = (e) => e.type === 'checkbox' || e.type === 'radio';

/** The field's kind: an input by its input type (text, date, password), anything else by its type. */
const fieldKind = (e) => (e.type === 'input' ? e.inputType || 'text' : e.type);

/** State words: required, checked, disabled, read-only, off-screen, ARIA state and covered, and invalid with the page's error. */
function fieldState(e) {
  const parts = [e.required && 'required', isToggle(e) && (e.checked ? 'checked' : 'unchecked')];
  parts.push(e.disabled && 'disabled', e.readOnly && 'readonly', !e.visible && 'off-screen', e.state);
  if (e.invalid || e.error) parts.push(e.error ? `invalid: ${e.error}` : 'invalid');
  return parts.filter(Boolean).join(' ');
}

/** The format a field expects, or a select's options; blank when it only repeats the label. */
function fieldHint(e) {
  const hint = e.options || e.placeholder || '';
  return hint === e.text ? '' : hint;
}

/** One row of the field table. */
const fieldRow = (e) => ({
  id: e.id,
  type: fieldKind(e),
  label: e.text,
  value: isToggle(e) ? '' : e.value,
  hint: fieldHint(e),
  state: fieldState(e),
});

/** One row for a visible element that is not a field. */
const visibleRow = (e) => ({
  id: e.id,
  type: e.type,
  label: [e.text || '', e.disabled && '(disabled)', e.state && `(${e.state})`].filter(Boolean).join(' '),
  link: (e.href || '').slice(0, PAGE_RENDER.MAX_INDEX_LINK),
});

/** The first off-screen elements, and a count of the rest. */
function offscreenSection(offscreen) {
  const listed = PAGE_RENDER.MAX_OFFSCREEN_LISTED;
  const rows = offscreen.slice(0, listed).map((e) => ({ id: e.id, type: e.type, label: e.text }));
  let section = '\nOff-screen (scroll to reveal):\n' + toonTable('offscreen', ['id', 'type', 'label'], rows) + '\n';
  if (offscreen.length > listed) section += `  ... and ${offscreen.length - listed} more off-screen elements\n`;
  return section;
}

/** Every field, the visible ones first: the whole form in one table. */
function fieldsSection(elements) {
  const fields = elements.filter((e) => FIELD_TYPES.has(e.type));
  if (!fields.length) return '';
  fields.sort((a, b) => Number(!a.visible) - Number(!b.visible));
  return FIELDS_NOTE + toonTable('fields', FIELD_COLUMNS, fields.map(fieldRow)) + '\n';
}

/** The visible elements that are not fields. */
function visibleSection(others) {
  const visible = others.filter((e) => e.visible);
  if (!visible.length) return '';
  return '\nOther visible elements:\n' + toonTable('visible', VISIBLE_COLUMNS, visible.map(visibleRow)) + '\n';
}

/** The index for an analysis's elements, noting when the page content was truncated. */
function elementIndex(elements, truncated) {
  const others = elements.filter((e) => !FIELD_TYPES.has(e.type));
  const offscreen = others.filter((e) => !e.visible);
  let index = `\n\n## Element Index (${elements.length} total, ${elements.filter((e) => e.visible).length} visible)\n\n`;
  index += fieldsSection(elements) + visibleSection(others);
  if (offscreen.length) index += offscreenSection(offscreen);
  return truncated ? index + TRUNCATED : index;
}

/** What the agent reads: the page's markdown within its budget, then the element index, always whole. */
function forAgent(analysis, max) {
  const index = elementIndex(analysis.elements || [], analysis.truncated);
  return fit(render(analysis), max - index.length) + index;
}

/** How the agent is told to read this format, for its system prompt. */
const guide = [
  '- analyze_page returns the page as markdown: a header of facts (url, title, scroll; and when they apply: panelScroll, the content scrolls inside a panel so scroll to see more even at 0%; modal, only that dialog was read; covered, something drawn over elements must be closed first; truncated), then the page.',
  '- Elements appear as [#id kind "name" → where a link goes, or = "what a field holds" (state)]; an element with no name has no name. State words: checked/unchecked, disabled, required, readonly, invalid with the page message, expanded/collapsed, selected, current, pressed, covered, off-screen, hint (a field format or a select options).',
  '- The Element Index after the page lists form fields (label, value, hint, state), then other visible elements, then off-screen ones to scroll to.',
].join('\n');

module.exports = { format: 'markdown', render, fit, forAgent, guide, elementIndex };
