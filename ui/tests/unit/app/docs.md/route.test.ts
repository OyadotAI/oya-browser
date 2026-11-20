/**
 * Unit tests for /docs.md: the whole docs page as Markdown, cut from the page
 * people read, with its code, tables and links intact.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { GET } from '@/app/docs.md/route';
import { serveDocsHtml } from '../../support/docs-html';

afterEach(() => vi.unstubAllGlobals());

describe('GET /docs.md', () => {
  it('carries the desktop-first setup and bounded detection claims from rendered HTML into agent docs', async () => {
    serveDocsHtml();
    const body = await (await GET()).text();
    expect(body.indexOf('### Desktop app')).toBeLessThan(body.indexOf('### Oya Cloud: servers, containers and CI'));
    expect(body).toMatch(/start a new\s+agent session/);
    expect(body).toContain('Sites can still require verification or block automation');
    expect(body).not.toContain('never flagged as a bot');
    expect(body).toContain('[Oya for coding agents](https://oyabrowser.com/browser-for-claude-code)');
  });

  it('serves the whole page as Markdown, pointing at the HTML as canonical', async () => {
    serveDocsHtml();
    const res = await GET();
    expect(res.status).toBe(200);
    expect(res.headers.get('content-type')).toBe('text/markdown; charset=utf-8');
    expect(res.headers.get('link')).toBe('<https://oyabrowser.com/docs>; rel="canonical"');
    const body = await res.text();
    expect(body).toContain('# A real browser for your agents.');
    for (const heading of ['## Quickstart', '## Playbooks', '## REST API', '## WebSocket Protocol'])
      expect(body).toContain(heading);
  });

  it('keeps code blocks, tables and links, and drops the page chrome', async () => {
    serveDocsHtml();
    const body = await (await GET()).text();
    expect(body).toContain('```\nnpm i @oya-ai/browser\n```');
    expect(body).toMatch(/^\| .+ \|\n\| --- \|/m);
    expect(body).toContain('[console](https://oyabrowser.com/dashboard)');
    expect(body).not.toContain('View as Markdown');
    expect(body).not.toMatch(/^Copy$/m);
  });

  it('answers 502 when the docs page cannot be fetched', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('refused')));
    expect((await GET()).status).toBe(502);
  });
});
