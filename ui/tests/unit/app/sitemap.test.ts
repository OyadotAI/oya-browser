/**
 * Unit tests for sitemap.xml: canonical HTML routes only, deterministic output
 * (no runtime new Date()), no invented editorial dates, and /pricing included.
 */
import { describe, it, expect } from 'vitest';
import sitemap from '@/app/sitemap';
import { SITE_URL } from '@/lib/site';
import { DOC_TOPICS } from '@/app/docs/_docs/pages';
import { ANSWER_PAGES } from '@/app/_answers/content';

describe('sitemap', () => {
  it('lists canonical HTML routes including /pricing and omits non-canonical or thin files', () => {
    const urls = sitemap().map((entry) => entry.url);
    expect(urls).toContain(`${SITE_URL}/`);
    expect(urls).toContain(`${SITE_URL}/pricing`);
    expect(urls).toContain(`${SITE_URL}/docs`);
    for (const { slug } of DOC_TOPICS) expect(urls).toContain(`${SITE_URL}/docs/${slug}`);
    for (const { path } of ANSWER_PAGES) expect(urls).toContain(`${SITE_URL}${path}`);
    expect(new Set(urls).size).toBe(urls.length);
    expect(urls.filter((url) => url.endsWith('.md'))).toEqual([]);
    for (const excluded of [
      '/docs/api/reference',
      '/docs/control-plane',
      '/llms.txt',
      '/llms-full.txt',
      '/openapi.json',
    ]) {
      expect(urls).not.toContain(`${SITE_URL}${excluded}`);
    }
  });

  it('omits request, experiment and vendor-check dates because they do not establish page updates', () => {
    const first = sitemap();
    const second = sitemap();
    expect(first).toEqual(second);
    for (const entry of first) expect(entry).not.toHaveProperty('lastModified');
  });
});
