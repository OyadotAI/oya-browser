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

  it('lets the page reach PostHog only when the operator set its host', () => {
    expect(directives(contentSecurityPolicy('n', false))['connect-src']).toBe("'self'");
    vi.stubEnv('POSTHOG_HOST', 'https://ph.example.test/');
    expect(directives(contentSecurityPolicy('n', false))['connect-src']).toBe("'self' https://ph.example.test");
  });

  it("lets the page fetch its remote config from PostHog cloud's assets host", () => {
    vi.stubEnv('POSTHOG_HOST', 'https://us.i.posthog.com');
    expect(directives(contentSecurityPolicy('n', false))['connect-src']).toBe(
      "'self' https://us.i.posthog.com https://us-assets.i.posthog.com",
    );
  });

  it('lets the page reach Sentry only when the operator set a DSN', () => {
    expect(directives(contentSecurityPolicy('n', false))['connect-src']).toBe("'self'");
    vi.stubEnv('SENTRY_DSN', 'https://abc@o1.ingest.us.sentry.io/42');
    expect(directives(contentSecurityPolicy('n', false))['connect-src']).toBe("'self' https://o1.ingest.us.sentry.io");
  });

  it('lets the page reach RB2B and its identity partners only when the operator set its account', () => {
    const rb2b =
      'https://app.rb2b.com https://9xgnrndqve.execute-api.us-west-2.amazonaws.com https://pro.ip-api.com ' +
      'https://a.usbrowserspeed.com https://alocdn.com https://*.liadm.com';
    expect(directives(contentSecurityPolicy('n', false))['connect-src']).toBe("'self'");
    expect(directives(contentSecurityPolicy('n', false))['img-src']).toBe("'self' data: blob:");
    vi.stubEnv('RB2B_ID', 'ABC123DEF456');
    expect(directives(contentSecurityPolicy('n', false))['connect-src']).toBe(`'self' ${rb2b}`);
    expect(directives(contentSecurityPolicy('n', false))['img-src']).toBe(`'self' data: blob: ${rb2b}`);
  });

  it("lets the page reach Meta's pixel only when the operator set one", () => {
    const meta = 'https://connect.facebook.net https://www.facebook.com';
    expect(directives(contentSecurityPolicy('n', false))['img-src']).toBe("'self' data: blob:");
    vi.stubEnv('META_PIXEL_ID', '123456789012345');
    expect(directives(contentSecurityPolicy('n', false))['connect-src']).toBe(`'self' ${meta}`);
    expect(directives(contentSecurityPolicy('n', false))['img-src']).toBe(`'self' data: blob: ${meta}`);
  });

  it('lets only the captcha, the walkthrough video and the sales calendar draw frames', () => {
    expect(directives(contentSecurityPolicy('n', false))['frame-src']).toBe(
      'https://challenges.cloudflare.com https://www.loom.com https://calendly.com',
    );
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
      'frame-src',
      'connect-src',
      'frame-ancestors',
      'object-src',
      'base-uri',
      'form-action',
    ]);
  });
});
