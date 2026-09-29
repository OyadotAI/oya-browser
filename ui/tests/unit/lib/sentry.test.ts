/**
 * Unit tests for browser error reporting settings: the DSN is read at request
 * time, its origin feeds the CSP, and key-shaped strings never leave the page.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import type { ErrorEvent } from '@sentry/browser';
import { scrub, sentryDsn, sentryOrigin } from '@/lib/sentry';

afterEach(() => vi.unstubAllEnvs());

describe('sentry settings', () => {
  it('is off when the operator set no DSN', () => {
    expect(sentryDsn()).toBeNull();
    expect(sentryOrigin()).toBeNull();
  });

  it('ignores a malformed DSN instead of breaking the page', () => {
    expect(sentryOrigin('not a url')).toBeNull();
  });

  it('redacts a key that ended up in an error', () => {
    const event = { message: 'Key Ab3dEf6hIj9kLm2nOp5qRs8tUv1wXy4z refused' } as ErrorEvent;
    expect(scrub(event).message).toBe('Key [redacted] refused');
  });
});
