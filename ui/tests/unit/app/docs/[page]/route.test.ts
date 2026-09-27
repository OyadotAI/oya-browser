/**
 * Unit tests for /docs/<slug>.md: every part of the docs has a Markdown twin
 * with its title and its real content, and nothing else under /docs answers.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { GET } from '@/app/docs/[page]/route';
import { DOC_PAGES } from '@/app/docs/_docs/pages';
import { serveDocsHtml } from '../../../support/docs-html';

afterEach(() => vi.unstubAllGlobals());

/** Asks the route for one path segment under /docs/. */
const get = (page: string) => GET(new Request(`http://localhost/docs/${page}`), { params: Promise.resolve({ page }) });

describe('GET /docs/<slug>.md', () => {
  for (const { slug, title } of DOC_PAGES) {
    it(`serves ${slug}.md as Markdown under its title`, async () => {
      serveDocsHtml();
      const res = await get(`${slug}.md`);
      expect(res.status).toBe(200);
      expect(res.headers.get('content-type')).toBe('text/markdown; charset=utf-8');
      const body = await res.text();
      expect(body.startsWith(`# ${title}\n`)).toBe(true);
      expect(body).toMatch(/^## \S/m);
      expect(body.replace(/```[\s\S]*?```/g, '')).not.toMatch(/<\/?(div|p|span|a|pre|code|section)[ >]/);
    });
  }

  it('cuts each part from its own section only', async () => {
    serveDocsHtml();
    const sdk = await (await get('sdk.md')).text();
    expect(sdk).toContain('## SDK');
    expect(sdk).not.toContain('## Quickstart');
  });

  it('answers an unknown page with a 404, without rendering anything', async () => {
    const fetch = serveDocsHtml();
    expect((await get('nope.md')).status).toBe(404);
    expect((await get('__proto__.md')).status).toBe(404);
    expect(fetch).not.toHaveBeenCalled();
  });

  it('answers a path without .md with a 404', async () => {
    serveDocsHtml();
    expect((await get('sdk')).status).toBe(404);
  });

  it('answers 502 when the docs page cannot be rendered', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('down', { status: 500 })),
    );
    expect((await get('sdk.md')).status).toBe(502);
  });
});
