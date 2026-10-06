/**
 * CHANGELOG.md read into releases for the release notes page. The file is ours and
 * keeps to one shape: `## <release>` headings, `### <group>` headings, `- ` items,
 * and prose paragraphs; inside a line, **bold**, `code` and [links](url). Anything
 * else is kept as prose rather than guessed at.
 */

/** One group of changes in a release: Added, Changed, Fixed. */
export interface ChangeGroup {
  /** The group's heading. */
  title: string;
  /** Its items, as written. */
  items: string[];
}

/** One release, or a section like Unreleased. */
export interface Release {
  /** The heading: `1.0.130 · 2026-09-26`, `Unreleased`. */
  title: string;
  /** Paragraphs before its first group. */
  notes: string[];
  /** Its groups of changes. */
  groups: ChangeGroup[];
}

/** One piece of a line: plain text, bold, code, or a link. */
export interface Inline {
  /** What the piece is. */
  kind: 'text' | 'bold' | 'code' | 'link';
  /** Its words. */
  text: string;
  /** Where a link goes. */
  href?: string;
}

/** The markers that start a release, a group and an item. */
const RELEASE = '## ';
const GROUP = '### ';
const ITEM = '- ';

/** Adds one line to the release being read. */
function readLine(releases: Release[], line: string) {
  const release = releases.at(-1);
  if (line.startsWith(RELEASE))
    return void releases.push({ title: line.slice(RELEASE.length).trim(), notes: [], groups: [] });
  if (!release || !line.trim()) return;
  if (line.startsWith(GROUP)) return void release.groups.push({ title: line.slice(GROUP.length).trim(), items: [] });
  const group = release.groups.at(-1);
  if (line.startsWith(ITEM) && group) return void group.items.push(line.slice(ITEM.length).trim());
  release.notes.push(line.trim());
}

/** The releases in a changelog, newest first as written; text before the first release is left out. */
export function parseChangelog(markdown: string): Release[] {
  const releases: Release[] = [];
  for (const line of markdown.split('\n')) readLine(releases, line);
  return releases;
}

/** The note a release gets when it was cut with nothing written for it (scripts/changelog.mjs NO_NOTES). */
const PLACEHOLDER = 'Maintenance and fixes.';

/**
 * Whether a release tells a reader anything: an empty Unreleased, and a release that
 * only carries the placeholder note, are left off the page.
 */
export const hasNews = (release: Release) =>
  release.notes.length > 0 || release.groups.some((g) => g.items.some((item) => item !== PLACEHOLDER));

/** **bold**, `code` and [text](url), in that order of appearance. */
const INLINE = /\*\*([^*]+)\*\*|`([^`]+)`|\[([^\]]+)\]\(([^)\s]+)\)/g;

/** A line split into plain text, bold, code and links. */
export function inlineOf(line: string): Inline[] {
  const parts: Inline[] = [];
  let at = 0;
  for (const m of line.matchAll(INLINE)) {
    parts.push(...textBetween(line, at, m.index), inlinePart(m));
    at = m.index + m[0].length;
  }
  return [...parts, ...textBetween(line, at, line.length)];
}

/** The plain text between two pieces, if there is any. */
const textBetween = (line: string, from: number, to: number): Inline[] =>
  to > from ? [{ kind: 'text', text: line.slice(from, to) }] : [];

/** One matched piece as bold, code or a link. */
function inlinePart(m: RegExpMatchArray): Inline {
  if (m[1] !== undefined) return { kind: 'bold', text: m[1] };
  if (m[2] !== undefined) return { kind: 'code', text: m[2] };
  return { kind: 'link', text: m[3], href: m[4] };
}
