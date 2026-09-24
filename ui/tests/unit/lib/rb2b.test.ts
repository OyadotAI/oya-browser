/**
 * Unit tests for loading RB2B: its script is added once, from RB2B's host, and
 * only for a well-formed account id.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { loadRb2b, validRb2bId } from '@/lib/rb2b';

/** RB2B scripts on the page. */
const scripts = () => [...document.querySelectorAll('script')].map((s) => s.src);

describe('loadRb2b', () => {
  beforeEach(() => {
    document.head.innerHTML = '';
    delete (window as unknown as { reb2b?: unknown }).reb2b;
    vi.useFakeTimers({ now: new Date('2026-09-24T16:00:00Z'), toFake: ['Date'] });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("adds RB2B's script for the account, once per page, versioned by the day so a settings change reaches returning visitors", () => {
    loadRb2b('1N5W0HJMYRO5');
    loadRb2b('1N5W0HJMYRO5');
    expect(scripts()).toEqual([
      'https://b2bjsstore.s3.us-west-2.amazonaws.com/b/1N5W0HJMYRO5/1N5W0HJMYRO5.js.gz?v=2026-09-24',
    ]);
  });

  it('adds nothing for an id that is not an RB2B account id', () => {
    loadRb2b('x"/><script>');
    expect(scripts()).toEqual([]);
  });

  it('accepts only upper-case letters and digits as an account id', () => {
    expect(validRb2bId('1N5W0HJMYRO5')).toBe(true);
    expect(validRb2bId(undefined)).toBe(false);
    expect(validRb2bId('abc')).toBe(false);
    expect(validRb2bId('../evil/x')).toBe(false);
  });
});
