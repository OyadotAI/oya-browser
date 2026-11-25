/**
 * Unit tests for /llm-full.txt: the singular name serves exactly what /llms-full.txt serves.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { GET } from '@/app/llm-full.txt/route';
import { GET as getLlmsFull } from '@/app/llms-full.txt/route';
import { serveDocsHtml } from '../../support/docs-html';

afterEach(() => vi.unstubAllGlobals());

describe('GET /llm-full.txt', () => {
  it('serves the same plain text as /llms-full.txt, with 200', async () => {
    serveDocsHtml();
    const res = await GET();
    expect(res.status).toBe(200);
    expect(res.headers.get('content-type')).toBe('text/plain; charset=utf-8');
    expect(await res.text()).toBe(await (await getLlmsFull()).text());
  });
});
