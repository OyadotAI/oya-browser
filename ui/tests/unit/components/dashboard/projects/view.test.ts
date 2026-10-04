/**
 * Unit tests for the switcher's derived text: how a key's expiry reads.
 */
import { describe, it, expect } from 'vitest';
import { expiryText } from '@/components/dashboard/projects/view';

/** A fixed now, between the two dates the tests use. */
const NOW = Date.parse('2026-06-01T00:00:00.000Z');

describe('expiryText', () => {
  it('says a key without an expiry never expires', () => {
    expect(expiryText(null, NOW)).toBe('Never expires');
    expect(expiryText(undefined, NOW)).toBe('Never expires');
  });

  it('says when a key will expire', () => {
    expect(expiryText('2027-01-05T12:00:00.000Z', NOW)).toMatch(/^Expires .*2027/);
  });

  it('says a key past its expiry has expired', () => {
    expect(expiryText('2026-01-05T12:00:00.000Z', NOW)).toMatch(/^Expired .*2026/);
  });
});
