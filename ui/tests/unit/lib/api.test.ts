/**
 * Unit tests for the API helpers and the account endpoints.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import {
  apiUrl,
  apiOrigin,
  authHeaders,
  login,
  refreshToken,
  logout,
  getProfile,
  listApiKeys,
  deleteApiKey,
  consoleCredential,
  CONSOLE_KEY,
} from '@/lib/api';
import { fakeFetch, fetchCall } from '../support';

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  sessionStorage.clear();
});

describe('URLs and headers', () => {
  it('prefixes paths with the same-origin API base', () => {
    expect(apiUrl('/fleet')).toBe('/api/fleet');
    expect(apiOrigin()).toBe(window.location.origin);
  });

  it('sends a bearer token as JSON and names the console as the client', () => {
    expect(authHeaders('t')).toEqual({
      Authorization: 'Bearer t',
      'Content-Type': 'application/json',
      'X-Oya-Client': 'console',
    });
  });
});

describe('account endpoints', () => {
  it('log in with the cookie credentials included', async () => {
    const fn = fakeFetch({ body: { access_token: 'a' } });
    expect(await login('e@x', 'pw')).toEqual({ access_token: 'a' });
    expect(fetchCall(fn)[1]).toMatchObject({
      method: 'POST',
      credentials: 'include',
      body: '{"email":"e@x","password":"pw"}',
    });
  });

  it("fail with the server's reason, else the endpoint's fallback", async () => {
    fakeFetch({ status: 401, body: { error: 'Wrong password' } }, { status: 500, body: 'not json' });
    await expect(login('e', 'p')).rejects.toThrow('Wrong password');
    await expect(login('e', 'p')).rejects.toThrow('Login failed');
  });

  it('refresh sends the fallback token only when one is given', async () => {
    const fn = fakeFetch({ body: {} });
    await refreshToken();
    await refreshToken('r1');
    expect(fetchCall(fn, 0)[1].body).toBe('{}');
    expect(fetchCall(fn, 1)[1].body).toBe('{"refresh_token":"r1"}');
  });

  it('logout never throws, even when the request fails', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('offline')));
    await expect(logout()).resolves.toBeUndefined();
  });

  it('rejects a profile that is not an object', async () => {
    fakeFetch({ body: 'null' });
    await expect(getProfile('t')).rejects.toThrow('Invalid profile response');
  });

  it('treats an empty key list as no keys', async () => {
    fakeFetch({ body: 'null' });
    expect(await listApiKeys('t')).toEqual([]);
  });

  it('encodes a key in the delete path, so a pasted ../ cannot redirect it', async () => {
    const fn = fakeFetch({ body: {} });
    await deleteApiKey('t', '../admin');
    expect(fetchCall(fn)[0]).toBe('/api/auth/keys/..%2Fadmin');
  });
});

describe('consoleCredential', () => {
  it('prefers the project credential, then the console key, else empty', () => {
    expect(consoleCredential()).toBe('');
    sessionStorage.setItem(CONSOLE_KEY, 'key');
    expect(consoleCredential()).toBe('key');
    sessionStorage.setItem('oya_project_credential', 'cred');
    expect(consoleCredential()).toBe('cred');
  });

  it('is empty when storage is blocked', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked');
    });
    expect(consoleCredential()).toBe('');
  });
});
