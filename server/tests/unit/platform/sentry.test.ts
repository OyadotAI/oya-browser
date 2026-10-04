/**
 * Unit tests for Sentry error reporting: it stays off without a DSN, and an
 * event loses secret headers, cookies, key-shaped strings, emails, long text
 * and console breadcrumbs before it leaves.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { scrub, startSentry } from '../../../src/platform/sentry.ts';

/** An API key as the server mints them: 32 base64url characters. */
const KEY = 'Ab3dEf6hIj9kLm2nOp5qRs8tUv1wXy4z';

describe('sentry', () => {
  it('does not start without a DSN, so a self-hosted server sends nothing', () => {
    assert.equal(startSentry({}), false);
  });

  it('redacts secret headers and drops cookies', () => {
    const event = scrub({
      request: { headers: { authorization: `Bearer ${KEY}`, 'x-api-key': 'k', accept: 'json' }, cookies: { s: '1' } },
    } as any);
    assert.deepEqual(event.request.headers, { authorization: '[redacted]', 'x-api-key': '[redacted]', accept: 'json' });
    assert.equal('cookies' in event.request, false);
  });

  it('redacts a key that ended up in an error message', () => {
    const event = scrub({ exception: { values: [{ type: 'Error', value: `Key ${KEY} was refused` }] } } as any);
    assert.equal(event.exception.values[0].value, 'Key [redacted] was refused');
  });

  it('keeps Sentry’s own ids, which are 32 characters too, so the event is accepted', () => {
    const id = 'f3a32feeeebc45baa1782a36c8937285';
    const event = scrub({ event_id: id, contexts: { trace: { trace_id: id, span_id: 'b'.repeat(16) } } } as any);
    assert.deepEqual([event.event_id, event.contexts.trace.trace_id], [id, id]);
  });

  it('redacts a key in a breadcrumb or request URL', () => {
    const event = scrub({
      request: { url: `https://x.test/?key=${KEY}` },
      breadcrumbs: [{ message: `sent ${KEY}`, data: { url: `https://y.test/${KEY}` } }],
    } as any);
    assert.equal(event.request.url, 'https://x.test/?key=[redacted]');
    assert.deepEqual(event.breadcrumbs[0], { message: 'sent [redacted]', data: { url: 'https://y.test/[redacted]' } });
  });

  it('redacts an email in an error message', () => {
    const event = scrub({
      exception: { values: [{ type: 'Error', value: 'No user jane.doe+x@clinic.example.org' }] },
    } as any);
    assert.equal(event.exception.values[0].value, 'No user [redacted]');
  });

  it('cuts long error text short, so a page quoted in an error does not leave', () => {
    const event = scrub({ message: `Command failed on: ${'patient notes '.repeat(100)}` } as any);
    assert.equal(event.message.length, 200);
  });

  it('drops console breadcrumbs, which echo whatever was logged, and keeps the rest', () => {
    const event = scrub({
      breadcrumbs: [
        { category: 'console', message: 'agent said something' },
        { category: 'http', data: { url: 'https://api.test/x' } },
      ],
    } as any);
    assert.deepEqual(
      event.breadcrumbs.map((b: { category: string }) => b.category),
      ['http'],
    );
  });
});
