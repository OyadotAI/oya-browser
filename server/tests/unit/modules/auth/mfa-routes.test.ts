/**
 * Unit tests for the MFA routes on a fake Supabase Auth client: who may call
 * them, enrolling, verifying (which starts a new session and is limited),
 * removing, and the audit trail each leaves.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { Status } from '../../../../src/platform/http-status.ts';
import { ownDataDir } from '../../support/data-dir.ts';
import { fakeRequest, routeThrough } from '../../support/auth.ts';
import { fakeMfaAuth, WRONG_CODE } from '../../support/mfa.ts';
import { recent } from '../../../../src/platform/audit.ts';
import { mfaClient } from '../../../../src/modules/auth/mfa.ts';

ownDataDir('oya-mfa-routes-');
const { router } = await import('../../../../src/modules/auth/routes.ts');

const realOpen = mfaClient.open;
afterEach(() => {
  mfaClient.open = realOpen;
});

/** Makes the routes open `fake` as the caller's client, and returns it. */
const using = (fake: any) => ((mfaClient.open = () => fake), fake);

/** POSTs to an MFA route as a signed-in person with a refresh cookie. */
const post = (path: string, body: any = {}, headers: any = {}) =>
  routeThrough(
    router,
    fakeRequest({
      method: 'POST',
      path: `/auth/mfa/${path}`,
      body,
      headers: { authorization: 'Bearer at', cookie: 'oya_rt=rt', ...headers },
    }),
  );

/** The newest audit row for `action` (recent() is newest first). */
const lastAudit = (action: string) => recent({ action })[0];

describe('who may use the MFA routes', () => {
  it('refuses a Login as request, so an admin never touches a customer factor', async () => {
    const res = await post('status', {}, { 'x-impersonate-token': 'imp' });
    assert.equal(res.statusCode, Status.FORBIDDEN);
  });

  it('asks to sign in again without a refresh token', async () => {
    const res = await routeThrough(
      router,
      fakeRequest({ method: 'POST', path: '/auth/mfa/status', headers: { authorization: 'Bearer at' } }),
    );
    assert.equal(res.statusCode, Status.UNAUTHORIZED);
  });

  it('takes the refresh token from the body when there is no cookie', async () => {
    const fake = using(fakeMfaAuth());
    const req = fakeRequest({ method: 'POST', path: '/auth/mfa/status', headers: { authorization: 'Bearer at' } });
    req.body = { refresh_token: 'rt-body' };
    await routeThrough(router, req);
    assert.equal(fake.calls[0][1].refresh_token, 'rt-body');
  });
});

describe('POST /auth/mfa/status', () => {
  it('answers the authenticators and the session level', async () => {
    using(fakeMfaAuth({ aal: 'aal1' }));
    const res = await post('status');
    assert.deepEqual(res.body, { factors: [], aal: 'aal1' });
  });
});

describe('POST /auth/mfa/enroll', () => {
  it('answers the QR code and secret, audited under the new factor', async () => {
    using(fakeMfaAuth());
    const res = await post('enroll');
    assert.equal(res.body.secret, 'SECRET');
    assert.deepEqual([lastAudit('auth.mfa.enroll').target_id, lastAudit('auth.mfa.enroll').outcome], ['f-new', 'ok']);
  });
});

describe('POST /auth/mfa/verify', () => {
  it('needs a factor and a six-digit code', async () => {
    using(fakeMfaAuth());
    assert.equal((await post('verify', { factor_id: 'f1', code: '12345' })).statusCode, Status.BAD_REQUEST);
    assert.equal((await post('verify', { code: '123456' })).statusCode, Status.BAD_REQUEST);
  });

  it('starts the two-factor session in the refresh cookie, audited as ok', async () => {
    using(fakeMfaAuth({ userId: 'u-ok' }));
    const res = await post('verify', { factor_id: 'f1', code: '123456' });
    assert.equal(res.cookies.oya_rt.value, 'rt-aal2');
    assert.equal(res.body.refresh_in_cookie, true);
    assert.deepEqual([lastAudit('auth.mfa.verify').outcome, lastAudit('auth.mfa.verify').target_id], ['ok', 'f1']);
  });

  it('refuses a wrong code with 400, audited as denied', async () => {
    using(fakeMfaAuth({ userId: 'u-wrong' }));
    const res = await post('verify', { factor_id: 'f1', code: WRONG_CODE });
    assert.deepEqual([res.statusCode, res.body.error], [Status.BAD_REQUEST, 'Invalid TOTP code entered']);
    assert.equal(lastAudit('auth.mfa.verify').outcome, 'denied');
  });

  it('limits code attempts per person with 429', async () => {
    using(fakeMfaAuth({ userId: 'u-limited' }));
    for (let i = 0; i < 10; i++) await post('verify', { factor_id: 'f1', code: WRONG_CODE });
    const res = await post('verify', { factor_id: 'f1', code: '123456' });
    assert.equal(res.statusCode, Status.TOO_MANY_REQUESTS);
  });
});

describe('POST /auth/mfa/unenroll', () => {
  it('needs a factor', async () => {
    using(fakeMfaAuth());
    assert.equal((await post('unenroll')).statusCode, Status.BAD_REQUEST);
  });

  it('removes the factor, audited', async () => {
    const fake = using(fakeMfaAuth());
    const res = await post('unenroll', { factor_id: 'f1' });
    assert.deepEqual(res.body, { ok: true });
    assert.deepEqual(fake.calls.at(-1), ['unenroll', 'f1']);
    assert.equal(lastAudit('auth.mfa.unenroll').target_id, 'f1');
  });
});
