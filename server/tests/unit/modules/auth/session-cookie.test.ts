/**
 * Unit tests for the session cookies: the refresh token moves into an httpOnly
 * cookie only for a same-origin caller, the readable hint beside it, reading
 * the cookie back, and clearing both.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { clearSessionCookies, issueSession, readRefreshCookie } from '../../../../src/modules/auth/session-cookie.ts';
import { SESSION_MAX_AGE_MS } from '../../../../src/modules/auth/constants.ts';
import { FakeResponse, fakeRequest } from '../../support/auth.ts';

/** What a login answers with. */
const SESSION = { user: { id: 'u1' }, access_token: 'at', refresh_token: 'rt', expires_at: 1 };

describe('issueSession', () => {
  it('moves the refresh token into an httpOnly cookie for a same-origin caller', () => {
    const res = new FakeResponse();
    issueSession(fakeRequest({ headers: { host: 'oya.example', origin: 'https://oya.example' } }), res, SESSION);
    assert.deepEqual(res.body, { user: { id: 'u1' }, access_token: 'at', expires_at: 1, refresh_in_cookie: true });
    assert.deepEqual(res.cookies.oya_rt, {
      value: 'rt',
      options: { httpOnly: true, sameSite: 'lax', secure: false, path: '/api/auth', maxAge: SESSION_MAX_AGE_MS },
    });
    assert.equal(res.cookies.oya_session.value, '1');
    assert.equal(res.cookies.oya_session.options.httpOnly, false, 'the hint is readable by the page');
  });

  it('treats a request without an Origin as same-origin', () => {
    const res = new FakeResponse();
    issueSession(fakeRequest({ headers: { host: 'oya.example' } }), res, SESSION);
    assert.equal(res.body.refresh_in_cookie, true);
  });

  it('marks the cookies secure behind an HTTPS proxy', () => {
    const res = new FakeResponse();
    issueSession(fakeRequest({ headers: { 'x-forwarded-proto': 'https' } }), res, SESSION);
    assert.equal(res.cookies.oya_rt.options.secure, true);
  });

  it('returns the token in the body to a cross-origin caller, which the cookie would never reach', () => {
    const res = new FakeResponse();
    issueSession(fakeRequest({ headers: { host: 'api.example', origin: 'http://localhost:3000' } }), res, SESSION);
    assert.deepEqual(res.body, SESSION);
    assert.deepEqual(res.cookies, {});
  });

  it('treats an unparseable Origin as cross-origin', () => {
    const res = new FakeResponse();
    issueSession(fakeRequest({ headers: { host: 'api.example', origin: 'not a url' } }), res, SESSION);
    assert.deepEqual(res.cookies, {});
  });

  it('sets no cookie when there is no refresh token', () => {
    const res = new FakeResponse();
    issueSession(fakeRequest(), res, { access_token: 'at' });
    assert.deepEqual([res.body, res.cookies], [{ access_token: 'at' }, {}]);
  });
});

describe('readRefreshCookie', () => {
  it('reads and decodes the refresh cookie among others', () => {
    const req = fakeRequest({ headers: { cookie: 'a=1; oya_rt=r%2Ft; oya_session=1' } });
    assert.equal(readRefreshCookie(req), 'r/t');
  });

  it('answers empty without the cookie, or with one that does not decode', () => {
    assert.equal(readRefreshCookie(fakeRequest()), '');
    assert.equal(readRefreshCookie(fakeRequest({ headers: { cookie: 'oya_rtx=1' } })), '');
    assert.equal(readRefreshCookie(fakeRequest({ headers: { cookie: 'oya_rt=%E0%A4%A' } })), '');
  });
});

describe('clearSessionCookies', () => {
  it('clears both cookies on the paths they were set on', () => {
    const res = new FakeResponse();
    clearSessionCookies(res);
    assert.deepEqual(res.cleared, { oya_rt: { path: '/api/auth' }, oya_session: { path: '/' } });
  });
});
