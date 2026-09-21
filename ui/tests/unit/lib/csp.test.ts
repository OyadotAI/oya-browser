/**
 * Unit tests for the Content-Security-Policy: scripts need the nonce, eval
 * only in development, and requests may reach an API on another origin.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { contentSecurityPolicy } from '@/lib/csp';

afterEach(() => vi.unstubAllEnvs());

/** The policy's directives by name. */
const directives = (policy: string) =>
  Object.fromEntries(policy.split('; ').map((d) => [d.split(' ')[0], d.slice(d.indexOf(' ') + 1)]));

describe('contentSecurityPolicy', () => {
  it('lets only nonce-bearing scripts run in production', () => {
    const d = directives(contentSecurityPolicy('abc', false));
    expect(d['script-src']).toBe("'self' 'nonce-abc' 'strict-dynamic'");
    expect(d['connect-src']).toBe("'self'");
  });

  it('allows eval and the dev socket in development only', () => {
    const d = directives(contentSecurityPolicy('abc', true));
    expect(d['script-src']).toContain("'unsafe-eval'");
    expect(d['connect-src']).toBe("'self' ws:");
  });

  it('allows requests to an API on another origin', () => {
    vi.stubEnv('NEXT_PUBLIC_API_URL', 'https://api.example.com/v1');
    expect(directives(contentSecurityPolicy('n', false))['connect-src']).toBe("'self' https://api.example.com");
  });

  it('keeps the directives in their established order', () => {
    const names = contentSecurityPolicy('n', false)
      .split('; ')
      .map((d) => d.split(' ')[0]);
    expect(names).toEqual([
      'default-src',
      'script-src',
      'style-src',
      'img-src',
      'media-src',
      'font-src',
      'connect-src',
      'frame-ancestors',
      'object-src',
      'base-uri',
      'form-action',
    ]);
  });
});
