/**
 * Unit tests for the request proxy: each request gets a fresh nonce, carried
 * to the render and matched by the policy the browser receives, and /docs
 * answers Markdown to a client that asks for it.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { NextRequest } from 'next/server';
import { proxy, config } from '@/proxy';

describe('proxy', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('passes a nonce to the render and sends the matching policy', async () => {
    const res = await proxy(new NextRequest('http://localhost/docs'));
    const csp = res.headers.get('content-security-policy')!;
    const forwarded = res.headers.get('x-middleware-request-x-nonce')!;
    expect(forwarded).toBeTruthy();
    expect(csp).toContain(`'nonce-${forwarded}'`);
  });

  it('uses a different nonce for every request', async () => {
    const a = (await proxy(new NextRequest('http://localhost/'))).headers.get('content-security-policy');
    const b = (await proxy(new NextRequest('http://localhost/'))).headers.get('content-security-policy');
    expect(a).not.toBe(b);
  });

  it('answers /docs with Markdown itself, without a rewrite, for a client that asks for it', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('<main><h1>Oya docs</h1></main>')),
    );
    const req = new NextRequest('https://localhost/docs', { headers: { accept: 'text/markdown' } });
    const res = await proxy(req);
    expect(res.headers.get('x-middleware-rewrite')).toBeNull();
    expect(res.headers.get('content-type')).toContain('text/markdown');
    expect(res.headers.get('vary')).toBe('Accept');
  });

  it('serves /docs as HTML to a browser, saying the answer varies on Accept', async () => {
    const res = await proxy(new NextRequest('http://localhost/docs', { headers: { accept: 'text/html,*/*' } }));
    expect(res.headers.get('x-middleware-rewrite')).toBeNull();
    expect(res.headers.get('vary')).toBe('Accept');
  });

  it('negotiates only on /docs', async () => {
    const res = await proxy(new NextRequest('http://localhost/', { headers: { accept: 'text/markdown' } }));
    expect(res.headers.get('x-middleware-rewrite')).toBeNull();
    expect(res.headers.get('vary')).toBeNull();
  });

  it('skips static assets and prefetches', () => {
    expect(config.matcher[0].source).toContain('_next/static');
    expect(config.matcher[0].missing).toEqual([{ type: 'header', key: 'next-router-prefetch' }]);
  });
});
