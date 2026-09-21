/**
 * Unit tests for the request proxy: each request gets a fresh nonce, carried
 * to the render and matched by the policy the browser receives.
 */
import { describe, it, expect } from 'vitest';
import { NextRequest } from 'next/server';
import { proxy, config } from '@/proxy';

describe('proxy', () => {
  it('passes a nonce to the render and sends the matching policy', () => {
    const res = proxy(new NextRequest('http://localhost/docs'));
    const csp = res.headers.get('content-security-policy')!;
    const forwarded = res.headers.get('x-middleware-request-x-nonce')!;
    expect(forwarded).toBeTruthy();
    expect(csp).toContain(`'nonce-${forwarded}'`);
  });

  it('uses a different nonce for every request', () => {
    const a = proxy(new NextRequest('http://localhost/')).headers.get('content-security-policy');
    const b = proxy(new NextRequest('http://localhost/')).headers.get('content-security-policy');
    expect(a).not.toBe(b);
  });

  it('skips static assets and prefetches', () => {
    expect(config.matcher[0].source).toContain('_next/static');
    expect(config.matcher[0].missing).toEqual([{ type: 'header', key: 'next-router-prefetch' }]);
  });
});
