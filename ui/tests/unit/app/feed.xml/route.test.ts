/**
 * Unit tests for /feed.xml: force-static build contract, valid RSS 2.0 XML,
 * deterministic output, absolute URLs and XML escaping.
 */
import { describe, it, expect } from 'vitest';
import { GET, dynamic } from '@/app/feed.xml/route';
import { buildFeedXml, escapeXml, wrapCdata } from '@/app/feed.xml/feed';
import { RESEARCH_POSTS } from '@/app/research/posts';
import { SITE_URL } from '@/lib/site';

describe('/feed.xml route', () => {
  it('is statically baked at build time so standalone containers never read CHANGELOG.md at runtime', () => {
    expect(dynamic).toBe('force-static');
  });

  it('returns valid RSS 2.0 XML with public caching, research posts and recent releases', async () => {
    const first = GET();
    const second = GET();
    expect(first.status).toBe(200);
    expect(first.headers.get('content-type')).toBe('application/rss+xml; charset=utf-8');
    expect(first.headers.get('cache-control')).toBe('public, max-age=300');
    const xml = await first.text();
    expect(xml).toBe(await second.text());
    expect(xml).toContain(
      '<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom" xmlns:content="http://purl.org/rss/1.0/modules/content/">',
    );
    expect(xml).toContain(`href="${SITE_URL}/feed.xml"`);
    for (const post of RESEARCH_POSTS) {
      expect(xml).toContain(`<link>${SITE_URL}${post.path}</link>`);
      expect(xml).toContain(`<title>${escapeXml(post.title)}</title>`);
      expect(xml).toContain(`<caption>${escapeXml(post.table.caption)}</caption>`);
    }
    expect(xml).toContain(`<link>${SITE_URL}/release-notes</link>`);
  });

  it('emits full <content:encoded> HTML for research posts and omits it on changelog releases', () => {
    const researchXml = buildFeedXml('', RESEARCH_POSTS);
    expect(researchXml).toContain('<content:encoded><![CDATA[');
    expect(researchXml).toContain('<table>');
    expect(researchXml).toContain(`<td>${escapeXml(RESEARCH_POSTS[0].table.rows[0][0])}</td>`);
    expect(researchXml).toContain(`<li>${escapeXml(RESEARCH_POSTS[0].notes[0])}</li>`);
    expect(researchXml).toContain(`<a href="${SITE_URL}${RESEARCH_POSTS[0].path}">`);
    const releaseXml = buildFeedXml('## 1.0.9 · 2026-10-01\n\n### Added\n- Fast replay', []);
    expect(releaseXml).not.toContain('<content:encoded>');
  });

  it('keeps CDATA well-formed by escaping HTML text and splitting raw CDATA end markers', () => {
    expect(wrapCdata('Safe ]]> still safe\u0007')).toBe('<![CDATA[Safe ]]]]><![CDATA[> still safe]]>');
    const hostile = { ...RESEARCH_POSTS[0], summary: 'Safe ]]> still safe' };
    const xml = buildFeedXml('', [hostile]);
    expect(xml).toContain('<p>Safe ]]&gt; still safe</p>');
    expect(xml).not.toContain('Safe ]]> still safe');
  });

  it('escapes XML special characters and strips illegal control characters', () => {
    expect(escapeXml('A & B <C> "D" \'E\'\u0007')).toBe('A &amp; B &lt;C&gt; &quot;D&quot; &apos;E&apos;');
    const xml = buildFeedXml(
      '## Unreleased\n\n### Added\n- Pending\n\n## 1.0.9 · 2026-10-01\n\n### Added\n- **Fast** & <safe> `replay`',
      [],
    );
    expect(xml).not.toContain('Unreleased');
    expect(xml).toContain('Fast &amp; &lt;safe&gt; replay');
    expect(xml).toContain('<lastBuildDate>Thu, 01 Oct 2026 00:00:00 GMT</lastBuildDate>');
  });

  it('does not present experiment dates as publication dates', () => {
    const xml = buildFeedXml('', RESEARCH_POSTS);
    expect(xml).toContain('<item>');
    expect(xml).not.toContain('<pubDate>');
    expect(xml).not.toContain('<lastBuildDate>');
  });
});
