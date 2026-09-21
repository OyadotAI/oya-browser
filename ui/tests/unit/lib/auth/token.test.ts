/**
 * Unit tests for token timing: reading a JWT's expiry and when to renew it.
 */
import { describe, it, expect } from 'vitest';
import { refreshDelay, tokenExpiry } from '@/lib/auth/token';
import { MIN_REFRESH_DELAY_MS } from '@/lib/constants';

/** An unsigned JWT with `payload`. */
const jwt = (payload: object) => `e30.${btoa(JSON.stringify(payload))}.sig`;

describe('tokenExpiry', () => {
  it('reads the exp claim', () => {
    expect(tokenExpiry(jwt({ exp: 1234 }))).toBe(1234);
  });

  it('is null without an exp claim or for something that is not a JWT', () => {
    expect(tokenExpiry(jwt({}))).toBeNull();
    expect(tokenExpiry('opaque')).toBeNull();
  });
});

describe('refreshDelay', () => {
  it('renews a minute before expiry', () => {
    expect(refreshDelay(1000, 0)).toBe((1000 - 60) * 1000);
  });

  it('never schedules sooner than the minimum, even for an expired token', () => {
    expect(refreshDelay(10, 1_000_000)).toBe(MIN_REFRESH_DELAY_MS);
  });
});
