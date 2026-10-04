/**
 * Unit tests for the second-factor rule in userAuthMiddleware, on a fake
 * Supabase user lookup: a session that has not passed a person's verified
 * authenticator opens no account route, while the MFA routes still take it
 * so the person can enter their code.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { Status } from '../../../../src/platform/http-status.ts';
import { ownDataDir } from '../../support/data-dir.ts';
import { fakeRequest, routeThrough, runMiddleware } from '../../support/auth.ts';
import { fakeMfaAuth, tokenWithAal } from '../../support/mfa.ts';

ownDataDir('oya-middleware-mfa-');
const { userAuthMiddleware, sessionUsers } = await import('../../../../src/modules/auth/middleware.ts');
const { mfaClient } = await import('../../../../src/modules/auth/mfa.ts');
const { router } = await import('../../../../src/modules/auth/routes.ts');

const real = { ...sessionUsers, open: mfaClient.open };
afterEach(() => {
  Object.assign(sessionUsers, { configured: real.configured, getUser: real.getUser });
  mfaClient.open = real.open;
});

/** A verified authenticator. */
const VERIFIED = { id: 'f1', factor_type: 'totp', status: 'verified' };

/** Makes Supabase know the token's owner as a person with `factors`. */
const supabaseKnows = (factors: any[]) =>
  Object.assign(sessionUsers, {
    configured: () => true,
    getUser: async () => ({ data: { user: { id: 'u1', email: 'u1@example.com', factors } }, error: null }),
  });

/** Runs userAuthMiddleware for a session at `aal`. */
const asSession = (aal: string) =>
  runMiddleware(userAuthMiddleware, fakeRequest({ headers: { authorization: `Bearer ${tokenWithAal(aal)}` } }));

describe('userAuthMiddleware and the second factor', () => {
  it('refuses an aal1 session of a person with a verified authenticator, with 401 mfa_required', async () => {
    supabaseKnows([VERIFIED]);
    const { res, passed } = await asSession('aal1');
    assert.deepEqual([passed, res.statusCode, res.body.code], [false, Status.UNAUTHORIZED, 'mfa_required']);
  });

  it('lets the same person in once the session passed the authenticator (aal2)', async () => {
    supabaseKnows([VERIFIED]);
    assert.equal((await asSession('aal2')).passed, true);
  });

  it('lets an aal1 session in when the person has no verified authenticator', async () => {
    supabaseKnows([{ ...VERIFIED, status: 'unverified' }]);
    assert.equal((await asSession('aal1')).passed, true);
  });

  it('still answers 401 for a token Supabase refuses', async () => {
    Object.assign(sessionUsers, { configured: () => true, getUser: async () => ({ data: null, error: {} }) });
    const { res } = await asSession('aal1');
    assert.deepEqual(res.body, { error: 'Invalid or expired token' });
  });

  it('keeps the MFA verify route open to that aal1 session, so the code can be entered', async () => {
    supabaseKnows([VERIFIED]);
    mfaClient.open = () => fakeMfaAuth({ factors: [VERIFIED], userId: 'u-step-up' }) as any;
    const req = fakeRequest({
      method: 'POST',
      path: '/auth/mfa/verify',
      body: { factor_id: 'f1', code: '123456' },
      headers: { authorization: `Bearer ${tokenWithAal('aal1')}`, cookie: 'oya_rt=rt' },
    });
    const res = await routeThrough(router, req);
    assert.deepEqual([res.statusCode, res.cookies.oya_rt.value], [200, 'rt-aal2']);
  });
});
