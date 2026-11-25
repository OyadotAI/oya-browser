/**
 * Pure RSS 2.0 feed builder combining dated research posts and recent CHANGELOG.md
 * releases into deterministic, properly escaped XML.
 */
import { SITE_DESCRIPTION, SITE_NAME, SITE_URL } from '@/lib/site';
import { hasNews, inlineOf, parseChangelog, type Release } from '../release-notes/changelog';
import type { ResearchPost } from '../research/posts';
import { MAX_CHANGELOG_FEED_ITEMS, MAX_RELEASE_SUMMARY_ITEMS } from './constants';

/** One item ready to serialize as an RSS <item>. */
interface FeedItem {
  /** Entry title. */
  title: string;
  /** Canonical absolute link. */
  link: string;
  /** Unique stable identifier. */
  guid: string;
  /** Whether guid is itself a permalink URL. */
  isPermaLink: boolean;
  /** Content event day used for sorting; an experiment date does not establish publication. */
  isoDate: string;
  /** Actual publication date when established by a release heading. */
  publishedDate?: string;
  /** Plain-text entry summary. */
  description: string;
  /** Full semantic HTML body emitted inside <content:encoded> for long-form posts. */
  contentHtml?: string;
}

/** ISO YYYY-MM-DD date matcher inside changelog release headings. */
const ISO_DATE = /\b(\d{4}-\d{2}-\d{2})\b/;

/** XML 1.0 illegal control characters stripped before serialization. */
const XML_CONTROL_CHARS = /[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F]/g;

/** Closing CDATA delimiter sequence split across sections to keep XML well-formed. */
const CDATA_END = /]]>/g;

/** Escapes XML text nodes and strips illegal XML 1.0 control characters. */
export function escapeXml(value: string): string {
  return value
    .replace(XML_CONTROL_CHARS, '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;');
}

/** Wraps HTML inside an XML CDATA section, stripping control chars and splitting "]]>". */
export const wrapCdata = (html: string): string =>
  `<![CDATA[${html.replace(XML_CONTROL_CHARS, '').replace(CDATA_END, ']]]]><![CDATA[>')}]]>`;

/** Static RSS 2.0 envelope opening lines, with site constants XML-escaped. */
const RSS_OPEN = [
  '<?xml version="1.0" encoding="UTF-8"?>',
  '<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom" xmlns:content="http://purl.org/rss/1.0/modules/content/">',
  '  <channel>',
  `    <title>${escapeXml(SITE_NAME)}</title>`,
  `    <link>${escapeXml(SITE_URL)}</link>`,
  `    <description>${escapeXml(SITE_DESCRIPTION)}</description>`,
  `    <atom:link href="${escapeXml(`${SITE_URL}/feed.xml`)}" rel="self" type="application/rss+xml" />`,
  '    <language>en-us</language>',
];

/** Static RSS 2.0 envelope closing lines. */
const RSS_CLOSE = ['  </channel>', '</rss>', ''];

/** Strips Markdown inline formatting into plain text. */
const plainText = (line: string): string =>
  inlineOf(line)
    .map((part) => part.text)
    .join('');

/** Converts an ISO YYYY-MM-DD date into an RFC 2822 UTC string for RSS pubDate. */
const rfc2822 = (isoDate: string): string => new Date(`${isoDate}T00:00:00Z`).toUTCString();

/** Builds a short plain-text summary from a changelog release's notes or first items. */
function releaseSummary(release: Release): string {
  const lines = release.notes.length > 0 ? release.notes : release.groups.flatMap((group) => group.items);
  return lines.slice(0, MAX_RELEASE_SUMMARY_ITEMS).map(plainText).join(' ');
}

/** Stable anchor identifier extracted from a changelog release heading. */
const releaseAnchor = (title: string, fallback: string): string =>
  title.split(/[\s·]+/)[0].replace(/[^a-zA-Z0-9._-]/g, '') || fallback;

/** Builds a FeedItem for a release once its heading date is verified. */
const makeReleaseItem = (title: string, isoDate: string, description: string): FeedItem => ({
  title: `${SITE_NAME} ${title}`,
  link: `${SITE_URL}/release-notes`,
  guid: `${SITE_URL}/release-notes#${releaseAnchor(title, isoDate)}`,
  isPermaLink: false,
  isoDate,
  publishedDate: isoDate,
  description,
});

/** Extracts a dated RSS item from a changelog release, or null when the heading has no date. */
function releaseToItem(release: Release): FeedItem | null {
  const title = plainText(release.title);
  const dateMatch = ISO_DATE.exec(title);
  return dateMatch ? makeReleaseItem(title, dateMatch[1], releaseSummary(release)) : null;
}

/** Renders one ResearchTable row as an HTML <tr> of escaped cells. */
const tableRow = (cells: readonly string[], tag: 'th' | 'td'): string =>
  `<tr>${cells.map((cell) => `<${tag}>${escapeXml(cell)}</${tag}>`).join('')}</tr>`;

/** Renders a ResearchPost table as semantic HTML for RSS readers and importers. */
const postTable = (table: ResearchPost['table']): string =>
  [
    '<table>',
    `<caption>${escapeXml(table.caption)}</caption>`,
    `<thead>${tableRow(table.columns, 'th')}</thead>`,
    `<tbody>${table.rows.map((row) => tableRow(row, 'td')).join('')}</tbody>`,
    '</table>',
  ].join('');

/** Renders a ResearchPost's caveat notes as an HTML unordered list. */
const postNotes = (notes: readonly string[]): string =>
  `<ul>${notes.map((note) => `<li>${escapeXml(note)}</li>`).join('')}</ul>`;

/** Builds full semantic HTML for a research post's <content:encoded> RSS body. */
const postHtml = (post: ResearchPost): string =>
  [
    `<p>${escapeXml(post.summary)}</p>`,
    `<p><strong>Measured:</strong> ${escapeXml(post.date)} · ${escapeXml(post.setup)}</p>`,
    `<p>${escapeXml(post.method)}</p>`,
    postTable(post.table),
    postNotes(post.notes),
    `<p><a href="${escapeXml(`${SITE_URL}${post.path}`)}">Read the full write-up on ${escapeXml(SITE_NAME)}</a>.</p>`,
  ].join('\n');

/** Converts a research post into an RSS item with full HTML content. */
const postToItem = (post: ResearchPost): FeedItem => ({
  title: post.title,
  link: `${SITE_URL}${post.path}`,
  guid: `${SITE_URL}${post.path}`,
  isPermaLink: true,
  isoDate: post.date,
  description: post.summary,
  contentHtml: postHtml(post),
});

/** Core identity lines shared by every RSS <item>. */
const itemHead = (item: FeedItem): string[] => [
  '    <item>',
  `      <title>${escapeXml(item.title)}</title>`,
  `      <link>${escapeXml(item.link)}</link>`,
  `      <guid isPermaLink="${item.isPermaLink}">${escapeXml(item.guid)}</guid>`,
];

/** Formats one FeedItem as an RSS <item> XML block. */
const renderItem = (item: FeedItem): string =>
  [
    ...itemHead(item),
    ...(item.publishedDate ? [`      <pubDate>${rfc2822(item.publishedDate)}</pubDate>`] : []),
    `      <description>${escapeXml(item.description)}</description>`,
    ...(item.contentHtml ? [`      <content:encoded>${wrapCdata(item.contentHtml)}</content:encoded>`] : []),
    '    </item>',
  ].join('\n');

/** Collects and sorts research posts and recent dated releases newest first. */
function collectItems(changelogMarkdown: string, posts: readonly ResearchPost[]): FeedItem[] {
  const releases = parseChangelog(changelogMarkdown)
    .filter(hasNews)
    .map(releaseToItem)
    .filter((item): item is FeedItem => item !== null)
    .slice(0, MAX_CHANGELOG_FEED_ITEMS);
  return [...posts.map(postToItem), ...releases].sort((a, b) => b.isoDate.localeCompare(a.isoDate));
}

/** Builds a deterministic RSS 2.0 XML document from research posts and dated changelog releases. */
export function buildFeedXml(changelogMarkdown: string, posts: readonly ResearchPost[]): string {
  const items = collectItems(changelogMarkdown, posts);
  const publishedDate = items.find((item) => item.publishedDate)?.publishedDate;
  const lastBuild = publishedDate ? [`    <lastBuildDate>${rfc2822(publishedDate)}</lastBuildDate>`] : [];
  return [...RSS_OPEN, ...lastBuild, ...items.map(renderItem), ...RSS_CLOSE].join('\n');
}
