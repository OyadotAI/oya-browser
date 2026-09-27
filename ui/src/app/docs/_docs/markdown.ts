/**
 * Turns the docs page's rendered HTML into Markdown, for agents. The Markdown
 * is cut from the same render people read, so the two cannot drift. It only
 * has to understand the markup the docs' own blocks produce: headings,
 * paragraphs, lists, tables, code, links and emphasis.
 */
import { SITE_URL } from '@/lib/site';
import { HEX_RADIX } from './constants';

/** One element of the parsed page. */
interface HtmlNode {
  /** The tag name, lower case. */
  tag: string;
  /** The raw attribute text. */
  attrs: string;
  /** Its elements and (decoded) text, in order. */
  children: (HtmlNode | string)[];
}

/** The docs split into their parts. */
export interface DocsMarkdown {
  /** The whole page. */
  full: string;
  /** Each part by its slug (the section's data-doc-page). */
  pages: Record<string, string>;
}

/** A comment, a tag (closing flag, name, attributes) or a run of text. */
const TOKEN = /<!--[\s\S]*?-->|<(\/?)([a-zA-Z][\w:-]*)((?:[^>"']|"[^"]*"|'[^']*')*)>|[^<]+/g;
/** Elements that never have a closing tag. */
const VOID = new Set(['br', 'hr', 'img', 'input', 'meta', 'link', 'source', 'wbr', 'area', 'col', 'embed', 'track']);
/** Named character references React emits. */
const ENTITIES: Record<string, string> = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: ' ' };
/** A fenced code block, kept apart from prose tidying. */
const FENCE = /(```[\s\S]*?```)/;

/** The text a character reference stands for, or undefined when unknown. */
function entity(ref: string): string | undefined {
  if (/^#x/i.test(ref)) return String.fromCodePoint(parseInt(ref.slice('#x'.length), HEX_RADIX));
  if (ref.startsWith('#')) return String.fromCodePoint(Number(ref.slice('#'.length)));
  return Object.hasOwn(ENTITIES, ref) ? ENTITIES[ref] : undefined;
}

/** Text with its character references resolved. */
function decode(text: string): string {
  return text.replace(/&(#x[\da-f]+|#\d+|\w+);/gi, (raw, ref: string) => entity(ref) ?? raw);
}

/** Opens an element under the current one; a void element takes no children. */
function openTag(stack: HtmlNode[], tag: string, attrs: string) {
  const node: HtmlNode = { tag, attrs, children: [] };
  stack.at(-1)!.children.push(node);
  if (!VOID.has(tag) && !attrs.trimEnd().endsWith('/')) stack.push(node);
}

/** Closes the nearest open element with this tag, and anything left open inside it. */
function closeTag(stack: HtmlNode[], tag: string) {
  const at = stack.findLastIndex((node) => node.tag === tag);
  if (at > 0) stack.length = at;
}

/** Applies one token to the tree being built. */
function applyToken(stack: HtmlNode[], [raw, closing, tag, attrs]: RegExpMatchArray) {
  if (raw.startsWith('<!--')) return;
  if (tag === undefined) return void stack.at(-1)!.children.push(decode(raw));
  if (closing) closeTag(stack, tag.toLowerCase());
  else openTag(stack, tag.toLowerCase(), attrs);
}

/** A tree of the markup. Enough for the well-formed HTML React writes, not for HTML in general. */
function parseHtml(html: string): HtmlNode {
  const root: HtmlNode = { tag: 'root', attrs: '', children: [] };
  const stack = [root];
  for (const token of html.matchAll(TOKEN)) applyToken(stack, token);
  return root;
}

/** An attribute's (decoded) value, or undefined. */
function attr(node: HtmlNode, name: string): string | undefined {
  const value = new RegExp(`(?:^|\\s)${name}="([^"]*)"`).exec(node.attrs)?.[1];
  return value === undefined ? undefined : decode(value);
}

/** The element children of a node. */
function elements(node: HtmlNode): HtmlNode[] {
  return node.children.filter((child): child is HtmlNode => typeof child !== 'string');
}

/** Every element under a node, depth first, that passes `test`. */
function findAll(node: HtmlNode, test: (el: HtmlNode) => boolean): HtmlNode[] {
  return elements(node).flatMap((el) => [...(test(el) ? [el] : []), ...findAll(el, test)]);
}

/** A node's text, exactly as written (for code). */
function textOf(node: HtmlNode | string): string {
  return typeof node === 'string' ? node : node.children.map(textOf).join('');
}

/** Content set apart as a block. */
const block = (content: string) => `\n\n${content.trim()}\n\n`;

/** A link's target as an absolute URL, so it still works outside the page. */
function absolute(href: string): string {
  return href.startsWith('#') ? `${SITE_URL}/docs${href}` : new URL(href, SITE_URL).href;
}

/** A link, or just its text when it has no target. */
function link(node: HtmlNode): string {
  const text = inner(node).trim();
  const href = attr(node, 'href');
  return href && text ? `[${text}](${absolute(href)})` : text;
}

/** A list, numbered or bulleted; a multi-line item is indented under its marker. */
function list(node: HtmlNode, ordered: boolean): string {
  const items = elements(node).filter((el) => el.tag === 'li');
  const lines = items.map((li, i) => `${ordered ? `${i + 1}.` : '-'} ${inner(li).trim().replace(/\n+/g, '\n  ')}`);
  return block(lines.join('\n'));
}

/** One table row, its cells kept on one line. */
function tableRow(cells: string[]): string {
  return `| ${cells.map((cell) => cell.replace(/\|/g, '\\|').replace(/\s*\n\s*/g, ' ')).join(' | ')} |`;
}

/** A table, its first row as the header. */
function table(node: HtmlNode): string {
  const rows = findAll(node, (el) => el.tag === 'tr').map((tr) => elements(tr).map((cell) => inner(cell).trim()));
  const [head = [], ...body] = rows;
  return block([tableRow(head), tableRow(head.map(() => '---')), ...body.map(tableRow)].join('\n'));
}

/** A heading at its level. */
function heading(node: HtmlNode): string {
  return block(`${'#'.repeat(Number(node.tag.slice(1)))} ${inner(node).trim()}`);
}

/** Wraps a node's content in a marker, unless it is empty. */
const wrap = (marker: string) => (node: HtmlNode) => {
  const text = inner(node).trim();
  return text ? `${marker}${text}${marker}` : '';
};

/** How each element becomes Markdown; any other element passes its content through. */
const RENDERERS: Record<string, (node: HtmlNode) => string> = {
  h1: heading,
  h2: heading,
  h3: heading,
  h4: heading,
  p: (node) => block(inner(node)),
  div: (node) => block(inner(node)),
  section: (node) => block(inner(node)),
  pre: (node) => block('```\n' + textOf(node).replace(/\n+$/, '') + '\n```'),
  code: (node) => '`' + textOf(node) + '`',
  kbd: (node) => '`' + textOf(node) + '`',
  strong: wrap('**'),
  b: wrap('**'),
  em: wrap('*'),
  i: wrap('*'),
  a: link,
  br: () => '\n',
  ul: (node) => list(node, false),
  ol: (node) => list(node, true),
  table,
  svg: () => '',
};

/** Whether a node is decoration or chrome that has no place in the Markdown. */
function skipped(node: HtmlNode): boolean {
  return attr(node, 'data-md') === 'skip' || attr(node, 'aria-hidden') === 'true';
}

/** One node as Markdown. */
function render(node: HtmlNode | string): string {
  if (typeof node === 'string') return node.replace(/\s+/g, ' ');
  if (skipped(node)) return '';
  return Object.hasOwn(RENDERERS, node.tag) ? RENDERERS[node.tag](node) : inner(node);
}

/** A node's children as Markdown. */
function inner(node: HtmlNode): string {
  return node.children.map(render).join('');
}

/** Prose with stray spaces around line breaks and runs of blank lines removed. */
function tidyProse(prose: string): string {
  return prose.replace(/[ \t]*\n[ \t]*/g, '\n').replace(/\n{3,}/g, '\n\n');
}

/** The final Markdown: prose tidied, code blocks left exactly as they are. */
function tidy(markdown: string): string {
  const parts = markdown.split(FENCE).map((part) => (part.startsWith('```') ? part : tidyProse(part)));
  return parts.join('').trim() + '\n';
}

/** The page's <main> element, parsed alone: the scripts around it are not HTML this parser can read. */
function parseMain(html: string): HtmlNode {
  const start = html.indexOf('<main');
  const end = html.indexOf('</main>', start);
  if (start < 0 || end < 0) throw new Error('The docs page has no <main>');
  return parseHtml(html.slice(start, end));
}

/** The docs page's HTML as Markdown: the whole <main>, and each <section data-doc-page> on its own. */
export function docsMarkdown(html: string): DocsMarkdown {
  const main = parseMain(html);
  const sections = findAll(main, (el) => attr(el, 'data-doc-page') !== undefined);
  const pages = Object.fromEntries(sections.map((el) => [attr(el, 'data-doc-page')!, tidy(render(el))]));
  return { full: tidy(render(main)), pages };
}
