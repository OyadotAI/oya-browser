/**
 * Unit tests for user accounts without Supabase configured: every account
 * operation answers with the status that says so rather than failing oddly.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  getProfile,
  login,
  oauthSignup,
  oauthUrl,
  refreshSession,
  signup,
  updateProfile,
} from '../../../../src/modules/auth/accounts.ts';
import { Status } from '../../../../src/platform/http-status.ts';

describe('accounts without Supabase', () => {
  it('refuses signup, login and refresh with 503', async () => {
    const unavailable = { status: Status.UNAVAILABLE, message: 'Database not configured' };
    await assert.rejects(signup('a@example.com', 'password1'), unavailable);
    await assert.rejects(login('a@example.com', 'password1'), unavailable);
    await assert.rejects(refreshSession('rt'), unavailable);
    await assert.rejects(oauthUrl('google', 'https://oyabrowser.com/auth/callback'), unavailable);
  });

  it('has no sign-in URL for a provider it does not offer', async () => {
    assert.equal(await oauthUrl('toString', 'https://oyabrowser.com/auth/callback'), null);
  });

  it('has no profile to read', async () => {
    assert.equal(await getProfile('u1'), null);
  });

  it('refuses a profile change with 409', async () => {
    await assert.rejects(updateProfile('u1', { display_name: 'Ada' }), {
      status: Status.CONFLICT,
      message: 'Accounts need Supabase',
    });
  });
});

describe('oauthSignup', () => {
  /** A Google account made `ago` milliseconds before now. */
  const made = (provider: string, ago: number) => ({ provider, created_at: new Date(NOW - ago).toISOString() });
  const NOW = Date.parse('2026-09-24T01:00:00Z');
  const MINUTE = 60_000;

  it('counts a Google or GitHub account made minutes ago as a sign-up by that provider', () => {
    assert.equal(oauthSignup(made('google', MINUTE), NOW), 'google');
    assert.equal(oauthSignup(made('github', MINUTE), NOW), 'github');
  });

  it('does not count a returning person, whose account is older', () => {
    assert.equal(oauthSignup(made('google', 60 * MINUTE), NOW), null);
  });

  it('does not count a password account or a provider it does not offer', () => {
    assert.equal(oauthSignup(made('email', MINUTE), NOW), null);
    assert.equal(oauthSignup(made('toString', MINUTE), NOW), null);
    assert.equal(oauthSignup({}, NOW), null);
  });
});
