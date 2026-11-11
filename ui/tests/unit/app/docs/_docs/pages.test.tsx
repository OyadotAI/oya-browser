/**
 * Unit tests for the docs' parts: each is a section of the page, and each
 * Markdown twin is listed where agents and crawlers look, llms.txt and the
 * sitemap.
 */
import { describe, it, expect, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { cleanup, render } from '@testing-library/react';
import DocsPage from '@/app/docs/page';
import sitemap from '@/app/sitemap';
import { DOC_PAGES, DOC_TOPICS, docPage, docTopic } from '@/app/docs/_docs/pages';

afterEach(cleanup);

/** llms.txt, which the server serves from its public folder (tests run from ui/). */
const LLMS = readFileSync(resolve(process.cwd(), '../server/src/public/llms.txt'), 'utf8');

describe('DOC_PAGES', () => {
  it('renders one section per part, in order', () => {
    render(<DocsPage />);
    const slugs = [...document.querySelectorAll('main section[data-doc-page]')].map((el) =>
      el.getAttribute('data-doc-page'),
    );
    expect(slugs).toEqual(DOC_PAGES.map((page) => page.slug));
  });

  it('lists every Markdown twin in llms.txt', () => {
    expect(LLMS).toContain('https://oyabrowser.com/docs.md');
    for (const { slug } of DOC_PAGES) expect(LLMS).toContain(`https://oyabrowser.com/docs/${slug}.md`);
  });

  it('lists every Markdown twin and llms-full.txt in the sitemap', () => {
    const urls = sitemap().map((entry) => entry.url);
    for (const path of ['/docs.md', '/llms-full.txt', ...DOC_PAGES.map(({ slug }) => `/docs/${slug}.md`)])
      expect(urls).toContain(`https://oyabrowser.com${path}`);
  });

  it('finds a part by slug, and nothing for an unknown one', () => {
    expect(docPage('sdk')?.title).toBe('SDK');
    expect(docPage('nope')).toBeUndefined();
  });
});

/** The one part with no page of its own: its section describes other vendors, so it waits for a rewrite. */
const HELD = 'control-plane';

describe('DOC_TOPICS', () => {
  it('gives every part its own page, except the one held back', () => {
    expect(DOC_TOPICS.map((page) => page.slug)).toEqual(DOC_PAGES.map((page) => page.slug).filter((s) => s !== HELD));
  });

  it('names each page differently, so no two share a title or a description', () => {
    expect(new Set(DOC_TOPICS.map((page) => page.topic.heading)).size).toBe(DOC_TOPICS.length);
    expect(new Set(DOC_TOPICS.map((page) => page.topic.description)).size).toBe(DOC_TOPICS.length);
  });

  it('finds a topic by slug, and nothing for the held part or an unknown one', () => {
    expect(docTopic('sdk')?.topic.heading).toBe('TypeScript SDK for browser agents');
    expect(docTopic(HELD)).toBeUndefined();
    expect(docTopic('nope')).toBeUndefined();
  });

  it('lists every topic page in the sitemap, and not the held part', () => {
    const urls = sitemap().map((entry) => entry.url);
    for (const { slug } of DOC_TOPICS) expect(urls).toContain(`https://oyabrowser.com/docs/${slug}`);
    expect(urls).not.toContain(`https://oyabrowser.com/docs/${HELD}`);
  });
});
