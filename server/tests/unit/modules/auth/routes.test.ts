/**
 * Unit tests for the auth routes without Supabase: input checks that answer
 * before any account call, logout clearing the cookies, and the key routes
 * refusing a caller without a signed-in session.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { Status } from '../../../../src/platform/http-status.ts';
import { ownDataDir } from '../../support/data-dir.ts';
import { fakeRequest, routeThrough } from '../../support/auth.ts';

ownDataDir('oya-auth-routes-');
const { router } = await import('../../../../src/modules/auth/routes.ts');

/** Sends one request through the auth router. */
const send = (method: string, path: string, body: any = {}, headers: any = {}) =>
  routeThrough(router, fakeRequest({ method, path, body, headers }));

describe('POST /auth/signup', () => {
  it('requires an email and a password', async () => {
    const res = await send('POST', '/auth/signup', { email: 'a@example.com' });
    assert.deepEqual([res.statusCode, res.body], [Status.BAD_REQUEST, { error: 'email and password required' }]);
  });

  it('requires a password of at least eight characters', async () => {
    const res = await send('POST', '/auth/signup', { email: 'a@example.com', password: 'short' });
    assert.equal(res.body.error, 'Password must be at least 8 characters');
  });

  it('answers an account failure with 400 and its message', async () => {
    const res = await send('POST', '/auth/signup', { email: 'a@example.com', password: 'long-enough' });
    assert.deepEqual([res.statusCode, res.body], [Status.BAD_REQUEST, { error: 'Database not configured' }]);
  });
});

describe('POST /auth/login', () => {
  it('requires an email and a password', async () => {
    assert.equal((await send('POST', '/auth/login', { password: 'x' })).statusCode, Status.BAD_REQUEST);
  });

  it('answers a failed sign-in with 401', async () => {
    const res = await send('POST', '/auth/login', { email: 'a@example.com', password: 'long-enough' });
    assert.equal(res.statusCode, Status.UNAUTHORIZED);
  });
});

describe('POST /auth/refresh', () => {
  it('requires a refresh token from the body or the cookie', async () => {
    const res = await send('POST', '/auth/refresh');
    assert.deepEqual([res.statusCode, res.body], [Status.BAD_REQUEST, { error: 'refresh_token required' }]);
  });

  it('clears the session cookies when the token is rejected', async () => {
    const res = await send('POST', '/auth/refresh', {}, { cookie: 'oya_rt=stale' });
    assert.equal(res.statusCode, Status.UNAUTHORIZED);
    assert.deepEqual(Object.keys(res.cleared).sort(), ['oya_rt', 'oya_session']);
  });
});

describe('POST /auth/logout', () => {
  it('clears the session cookies', async () => {
    const res = await send('POST', '/auth/logout');
    assert.deepEqual(res.body, { ok: true });
    assert.deepEqual(Object.keys(res.cleared).sort(), ['oya_rt', 'oya_session']);
  });
});

describe('signed-in routes', () => {
  for (const [method, path] of [
    ['GET', '/auth/me'],
    ['PATCH', '/auth/me'],
    ['GET', '/auth/keys'],
    ['POST', '/auth/keys'],
    ['POST', '/auth/keys/import'],
    ['DELETE', '/auth/keys/abc'],
  ]) {
    it(`${method} ${path} refuses a caller without a session token`, async () => {
      const res = await send(method, path);
      assert.deepEqual([res.statusCode, res.body], [Status.UNAUTHORIZED, { error: 'Missing token' }]);
    });
  }

  it('matches paths case-sensitively', async () => {
    assert.equal((await send('POST', '/AUTH/LOGOUT')).statusCode, Status.NOT_FOUND);
  });
});
