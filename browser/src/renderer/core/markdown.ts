/**
 * The agent's replies as HTML, from a small, escaped subset of Markdown:
 * paragraphs, headings, lists, tables, rules, code blocks, inline code, bold
 * and italic. Everything is escaped first, so only these tags are produced.
 */

/** A Markdown list item: `-`, `*`, `•` or `1.` / `1)` at the start of a line. */
const LIST_ITEM = /^\s*(?:[-*•]|\d+[.)])\s+/;

/** A Markdown heading line. */
const HEADING = /^#{1,6}\s+(.+)$/;

/** A Markdown rule: three or more dashes, stars or underscores alone on a line. */
const RULE = /^(?:-{3,}|\*{3,}|_{3,})$/;

/** The line under a Markdown table's header: cells of dashes, each with optional colons. */
const TABLE_DIVIDER = /^\|?\s*:?-+:?\s*(?:\|\s*:?-+:?\s*)*\|?$/;

/** Inline code, bold and italic, applied in order. */
const INLINE: readonly [RegExp, string][] = [
  [/`([^`]+)`/g, '<code>$1</code>'],
  [/\*\*(.+?)\*\*/g, '<strong>$1</strong>'],
  [/(?<![*\w])\*([^*\n]+)\*(?![*\w])/g, '<em>$1</em>'],
];

/** The characters HTML text escapes, as the DOM serializes text: &, <, > and the no-break space. */
const ESCAPES: Readonly<Record<string, string>> = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '\u00a0': '&nbsp;' };

/** `text` escaped for use inside HTML, exactly as the DOM serializes a text node. */
export const escapeHtml = (text: string): string => text.replace(/[&<>\u00a0]/g, (ch) => ESCAPES[ch]);

/** Inline code, bold and italic. */
const inline = (text: string): string => INLINE.reduce((s, [pattern, html]) => s.replace(pattern, html), text);

/** One table line's cells, with the outer pipes dropped. */
const cells = (line: string): string[] =>
  line
    .trim()
    .replace(/^\||\|$/g, '')
    .split('|')
    .map((cell) => cell.trim());

/** One table row of `tag` cells. */
const row = (items: string[], tag: string): string =>
  '<tr>' + items.map((c) => `<${tag}>${inline(c)}</${tag}>`).join('') + '</tr>';

/** A table: the header row, the divider skipped, then the body rows. Wrapped so a wide one scrolls. */
function table(lines: string[]): string {
  const [head, , ...rows] = lines.map(cells);
  const body = rows.map((items) => row(items, 'td')).join('');
  return `<div class="chat-table"><table><thead>${row(head, 'th')}</thead><tbody>${body}</tbody></table></div>`;
}

/** Consecutive lines grouped by whether they are list items. */
function runs(lines: string[]): string[][] {
  const grouped: string[][] = [];
  for (const line of lines) {
    const last = grouped.at(-1);
    if (last && LIST_ITEM.test(last[0]) === LIST_ITEM.test(line)) last.push(line);
    else grouped.push([line]);
  }
  return grouped;
}

/** A run as a list (numbered when it starts with a number) or a paragraph. */
function run(lines: string[]): string {
  if (!LIST_ITEM.test(lines[0])) return '<p>' + lines.map(inline).join('<br>') + '</p>';
  const tag = /^\s*\d/.test(lines[0]) ? 'ol' : 'ul';
  const items = lines.map((line) => '<li>' + inline(line.replace(LIST_ITEM, '')) + '</li>');
  return `<${tag}>${items.join('')}</${tag}>`;
}

/** A heading, a rule, a table, or runs of paragraph lines and list items. */
function block(text: string): string {
  const heading = HEADING.exec(text);
  if (heading) return '<h4>' + inline(heading[1]) + '</h4>';
  if (RULE.test(text)) return '<hr>';
  const lines = text.split('\n');
  if (TABLE_DIVIDER.test(lines[1] ?? '')) return table(lines);
  return runs(lines).map(run).join('');
}

/** Text between code blocks: blank lines, headings and rules start new blocks. */
function prose(text: string): string {
  const blocks = text.replace(/^(#{1,6}\s.+|-{3,}|\*{3,}|_{3,})$/gm, '\n$1\n').split(/\n\s*\n/);
  return blocks
    .map((b) => b.trim())
    .filter(Boolean)
    .map(block)
    .join('');
}

/** One fenced code block, kept verbatim. */
const codeBlock = (code: string): string => '<pre><code>' + code.replace(/\n$/, '') + '</code></pre>';

/** Escaped text with the Markdown subset turned into HTML. Code fences are split out first. */
export function markdownToHtml(text: string): string {
  // split() with a capture group alternates prose and code: code sits at the odd indexes.
  const parts = escapeHtml(text).split(/```[\w-]*\n?([\s\S]*?)```/);
  return parts.map((part, i) => (i & 1 ? codeBlock(part) : prose(part))).join('');
}
