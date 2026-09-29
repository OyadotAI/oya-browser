/**
 * Unit tests for Sentry error reporting: it stays off without a DSN, and an
 * event loses secret headers, cookies and key-shaped strings before it leaves.
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
});
