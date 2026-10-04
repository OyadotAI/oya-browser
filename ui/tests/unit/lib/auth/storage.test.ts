/**
 * Unit tests for what the session keeps in browser storage.
 */
import { describe, it, expect, afterEach } from 'vitest';
import {
  clearStoredSession,
  hasSessionCookie,
  impersonation,
  keepRefreshToken,
  keepStepUp,
  setImpersonation,
  stepUpPending,
  storedRefreshToken,
} from '@/lib/auth/storage';

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

  it('starts and ends a Login as, dropping the project credentials each time', () => {
    sessionStorage.setItem('oya_project_credential', 'admin-cred');
    setImpersonation({ token: 't', email: 'c@example.com' });
    expect(impersonation()).toEqual({ token: 't', email: 'c@example.com' });
    expect(sessionStorage.getItem('oya_project_credential')).toBeNull();
    sessionStorage.setItem('oya_project_credential', 'cust-cred');
    setImpersonation(null);
    expect([impersonation(), sessionStorage.length]).toEqual([null, 0]);
  });

  it('ends a Login as on sign-out, and reads a damaged one as none', () => {
    setImpersonation({ token: 't', email: 'c@example.com' });
    clearStoredSession();
    expect(impersonation()).toBeNull();
    sessionStorage.setItem('oya_impersonation', '{');
    expect(impersonation()).toBeNull();
  });
});

describe('the owed second factor', () => {
  it('remembers that the session owes its code until the server says otherwise', () => {
    expect(stepUpPending()).toBe(false);
    keepStepUp(true);
    expect(stepUpPending()).toBe(true);
    keepStepUp(false);
    expect(stepUpPending()).toBe(false);
  });

  it('forgets it on sign-out', () => {
    keepStepUp(true);
    clearStoredSession();
    expect(stepUpPending()).toBe(false);
  });
});
