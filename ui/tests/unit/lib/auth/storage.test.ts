/**
 * Unit tests for what the session keeps in browser storage.
 */
import { describe, it, expect, afterEach } from 'vitest';
import { clearStoredSession, hasSessionCookie, keepRefreshToken, storedRefreshToken } from '@/lib/auth/storage';

afterEach(() => {
  localStorage.clear();
  sessionStorage.clear();
  document.cookie = 'oya_session=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/';
});

describe('session storage', () => {
  it('keeps a returned refresh token and drops the stored one when none is returned', () => {
    keepRefreshToken('r1');
    expect(storedRefreshToken()).toBe('r1');
    keepRefreshToken(undefined);
    expect(storedRefreshToken()).toBeUndefined();
  });

  it('clears every key a session may have left, including the legacy API key', () => {
    for (const k of ['oya_token', 'oya_refresh_token', 'oya_api_key', 'other']) localStorage.setItem(k, 'x');
    for (const k of ['oya_console_key', 'oya_project_credential', 'oya_project_id']) sessionStorage.setItem(k, 'x');
    clearStoredSession();
    expect(Object.keys(localStorage)).toEqual(['other']);
    expect(sessionStorage.length).toBe(0);
  });

  it('sees the readable session marker cookie', () => {
    expect(hasSessionCookie()).toBe(false);
    document.cookie = 'oya_session=1; path=/';
    expect(hasSessionCookie()).toBe(true);
  });
});
