/** Native return validation permits only a bounded literal loopback callback and one-use code. */
import { describe, expect, it } from 'vitest';
import { desktopReturnFrom, desktopReturnUrl, validDesktopReturn } from '@/components/dashboard/desktop-return';

const state = 'a'.repeat(64);
const request = () => ({
  callback: 'http://127.0.0.1:41777/oya-desktop/callback',
  state,
  expiresAt: Date.now() + 10000,
});
describe('desktop return', () => {
  it('returns only the one-use pairing code and native state to literal loopback', () => {
    const url = new URL(desktopReturnUrl(request(), 'one-use-code'));
    expect(url.origin).toBe('http://127.0.0.1:41777');
    expect([...url.searchParams.keys()].sort()).toEqual(['code', 'state']);
    expect(url.searchParams.get('state')).toBe(state);
  });
  it('refuses arbitrary hosts, credentials, nested queries, wrong paths and expired requests', () => {
    for (const callback of [
      'https://evil.test/oya-desktop/callback',
      'http://localhost:41777/oya-desktop/callback',
      'http://127.0.0.1.evil.test:41777/oya-desktop/callback',
      'http://u:p@127.0.0.1:41777/oya-desktop/callback',
      'http://127.0.0.1:41777/other',
      'http://127.0.0.1:41777/oya-desktop/callback?next=x',
    ])
      expect(validDesktopReturn({ ...request(), callback })).toBe(false);
    expect(validDesktopReturn({ ...request(), expiresAt: Date.now() - 1 })).toBe(false);
    expect(validDesktopReturn({ ...request(), state: 'guessable' })).toBe(false);
  });
  it('remembers only a well-formed return visit for a bounded period', () => {
    const params = new URLSearchParams({ callback: request().callback, state });
    expect(desktopReturnFrom(params)?.state).toBe(state);
    params.set('callback', 'https://evil.test');
    expect(desktopReturnFrom(params)).toBeNull();
  });
});
