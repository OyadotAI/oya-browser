/**
 * Unit tests for two-factor sign-in on a fake Supabase Auth client: reading a
 * session's level, when a sign-in still owes its code, opening the caller's
 * client, and enrolling, listing, verifying and removing an authenticator.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import {
  aalOf,
  enrollTotp,
  mfaClient,
  mfaStatus,
  openMfa,
  owesSecondFactor,
  unenrollTotp,
  verifyTotp,
} from '../../../../src/modules/auth/mfa.ts';
import { Status } from '../../../../src/platform/http-status.ts';
import { fakeMfaAuth, tokenWithAal, WRONG_CODE } from '../../support/mfa.ts';

const realOpen = mfaClient.open;
afterEach(() => {
  mfaClient.open = realOpen;
});

/** A verified and an unverified authenticator. */
const VERIFIED = { id: 'f1', factor_type: 'totp', status: 'verified', friendly_name: 'Phone', created_at: 't' };
const PENDING = { id: 'f2', factor_type: 'totp', status: 'unverified', friendly_name: '', created_at: 't' };

describe('aalOf and owesSecondFactor', () => {
  it('reads the level a token carries, and nothing from a token that is not a JWT', () => {
    assert.equal(aalOf(tokenWithAal('aal2')), 'aal2');
    assert.equal(aalOf('not-a-jwt'), undefined);
  });

  it('owes a code only when the person has a verified factor and the token is aal1', () => {
    const user = { factors: [VERIFIED] };
    assert.equal(owesSecondFactor(user, tokenWithAal('aal1')), true);
    assert.equal(owesSecondFactor(user, tokenWithAal('aal2')), false);
    assert.equal(owesSecondFactor({ factors: [PENDING] }, tokenWithAal('aal1')), false);
    assert.equal(owesSecondFactor({}, tokenWithAal('aal1')), false);
  });
});

describe('openMfa', () => {
  it('asks to sign in again without both tokens', async () => {
    await assert.rejects(openMfa('at', ''), { status: Status.UNAUTHORIZED });
  });

  it('answers 503 without Supabase Auth', async () => {
    await assert.rejects(openMfa('at', 'rt'), { status: Status.UNAVAILABLE });
  });

  it('answers 401 when Supabase does not take the tokens', async () => {
    mfaClient.open = () => fakeMfaAuth({ rejectSession: true }) as any;
    await assert.rejects(openMfa('at', 'rt'), { status: Status.UNAUTHORIZED, message: 'Invalid or expired token' });
  });

  it('signs the client in as the caller with their two tokens', async () => {
    const fake = fakeMfaAuth();
    mfaClient.open = () => fake as any;
    const { user } = await openMfa('at', 'rt');
    assert.equal(user.id, 'u1');
    assert.deepEqual(fake.calls[0], ['setSession', { access_token: 'at', refresh_token: 'rt' }]);
  });
});

describe('authenticators', () => {
  it('lists only authenticator apps, with the session level', async () => {
    const phone = { id: 'p', factor_type: 'phone', status: 'verified' };
    const status = await mfaStatus(fakeMfaAuth({ factors: [VERIFIED, phone], aal: 'aal2' }));
    assert.deepEqual(status, {
      factors: [{ id: 'f1', name: 'Phone', status: 'verified', created_at: 't' }],
      aal: 'aal2',
    });
  });

  it('removes an abandoned enrolment before starting a new one, and answers the QR code and secret', async () => {
    const fake = fakeMfaAuth({ factors: [VERIFIED, PENDING] });
    const enrolled = await enrollTotp(fake);
    assert.deepEqual(enrolled, {
      id: 'f-new',
      qr_code: 'data:image/svg+xml;utf-8,<svg/>',
      secret: 'SECRET',
      uri: 'otpauth://totp/Oya',
    });
    assert.deepEqual(fake.calls[0], ['unenroll', 'f2']);
    assert.equal(fake.calls[1][1].issuer, 'Oya Browser');
  });

  it('answers a good code with the new two-factor session', async () => {
    const session = await verifyTotp(fakeMfaAuth(), 'f1', '123456');
    assert.equal(aalOf(session.access_token), 'aal2');
    assert.deepEqual([session.refresh_token, session.user.id], ['rt-aal2', 'u1']);
  });

  it('refuses a wrong code with 400 in Supabase words', async () => {
    await assert.rejects(verifyTotp(fakeMfaAuth(), 'f1', WRONG_CODE), {
      status: Status.BAD_REQUEST,
      message: 'Invalid TOTP code entered',
    });
  });

  it('removes the named authenticator', async () => {
    const fake = fakeMfaAuth({ factors: [VERIFIED] });
    await unenrollTotp(fake, 'f1');
    assert.deepEqual(fake.calls, [['unenroll', 'f1']]);
  });
});
