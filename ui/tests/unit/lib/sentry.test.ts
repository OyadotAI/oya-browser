/**
 * Unit tests for browser error reporting settings: the DSN is read at request
 * time, its origin feeds the CSP, and key-shaped strings, emails, long text and
 * console output never leave the page.
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

  it('keeps Sentry’s own ids, which are 32 characters too, so the event is accepted', () => {
    const id = 'f3a32feeeebc45baa1782a36c8937285';
    const event = { event_id: id, contexts: { trace: { trace_id: id } } } as unknown as ErrorEvent;
    expect(scrub(event).event_id).toBe(id);
  });

  it('redacts a key in a fetch breadcrumb URL', () => {
    const event = {
      breadcrumbs: [{ data: { url: '/api?k=Ab3dEf6hIj9kLm2nOp5qRs8tUv1wXy4z' } }],
    } as unknown as ErrorEvent;
    expect(scrub(event).breadcrumbs?.[0].data?.url).toBe('/api?k=[redacted]');
  });

  it('redacts an email in an error', () => {
    const event = { exception: { values: [{ value: 'No account for jane.doe+x@clinic.example.org' }] } } as ErrorEvent;
    expect(scrub(event).exception?.values?.[0].value).toBe('No account for [redacted]');
  });

  it('cuts long error text short, so a page quoted in an error does not leave', () => {
    const event = { message: `Failed on page: ${'patient notes '.repeat(100)}` } as ErrorEvent;
    expect(scrub(event).message?.length).toBe(200);
  });

  it('drops console breadcrumbs and keeps the rest', () => {
    const event = {
      breadcrumbs: [
        { category: 'console', message: 'page said something' },
        { category: 'fetch', data: { url: '/api' } },
      ],
    } as unknown as ErrorEvent;
    expect(scrub(event).breadcrumbs?.map((b) => b.category)).toEqual(['fetch']);
  });
});
