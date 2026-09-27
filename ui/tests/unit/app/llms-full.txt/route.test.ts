/**
 * Unit tests for /llms-full.txt: every docs page in one plain-text file.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { GET } from '@/app/llms-full.txt/route';
import { GET as getDocsMd } from '@/app/docs.md/route';
import { serveDocsHtml } from '../../support/docs-html';

afterEach(() => vi.unstubAllGlobals());

describe('GET /llms-full.txt', () => {
  it('serves the same text as /docs.md, as plain text', async () => {
    serveDocsHtml();
    const res = await GET();
    expect(res.headers.get('content-type')).toBe('text/plain; charset=utf-8');
    expect(await res.text()).toBe(await (await getDocsMd()).text());
  });
});
