/**
 * Unit tests for who may open the admin page: the domain rule, the explicit
 * OYA_ADMIN_EMAILS list, and the second factor OYA_ADMIN_REQUIRE_MFA asks for.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { adminOnly, isAdmin } from '../../../../src/modules/admin/access.ts';

/** A confirmed user with this email. */
const user = (email: string, confirmed = true) => ({
  email,
  email_confirmed_at: confirmed ? '2026-01-01T00:00:00Z' : null,
});

/** Runs adminOnly for a request; returns whether it passed and the refusal's status and body. */
function gate(req) {
  const res: any = { status: mock.fn(() => res), json: mock.fn() };
  let passed = false;
  adminOnly(req, res, () => (passed = true));
  return { passed, status: res.status.mock.calls[0]?.arguments[0], body: res.json.mock.calls[0]?.arguments[0] };
}

describe('admin access', () => {
  const saved = { list: process.env.OYA_ADMIN_EMAILS, mfa: process.env.OYA_ADMIN_REQUIRE_MFA };
  afterEach(() => {
    for (const [name, value] of [
      ['OYA_ADMIN_EMAILS', saved.list],
      ['OYA_ADMIN_REQUIRE_MFA', saved.mfa],
    ])
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
  });

  it('lets in a confirmed address at the company domain, whatever its case', () => {
    assert.equal(isAdmin(user('mk@getoya.ai')), true);
    assert.equal(isAdmin(user('MK@GetOya.AI')), true);
  });

  it('keeps out an unconfirmed address, another domain, a look-alike and nobody', () => {
    assert.equal(isAdmin(user('mk@getoya.ai', false)), false);
    assert.equal(isAdmin(user('mk@oya.ai')), false);
    assert.equal(isAdmin(user('mk@evil-getoya.ai')), false);
    assert.equal(isAdmin(user('a@b.com@getoya.ai')), false);
    assert.equal(isAdmin(undefined), false);
  });

  it('answers 403 to anyone else, and passes an admin on', () => {
    const res: any = { status: mock.fn(() => res), json: mock.fn() };
    let passed = false;
    adminOnly({ user: user('x@gmail.com') }, res, () => (passed = true));
    assert.equal(res.status.mock.calls[0].arguments[0], 403);
    adminOnly({ user: user('mk@getoya.ai') }, res, () => (passed = true));
    assert.equal(passed, true);
  });

  it('with OYA_ADMIN_EMAILS set, lets in only the confirmed addresses it lists', () => {
    process.env.OYA_ADMIN_EMAILS = ' Boss@getoya.ai , ops@example.com';
    assert.equal(isAdmin(user('boss@getoya.ai')), true);
    assert.equal(isAdmin(user('ops@example.com')), true);
    assert.equal(isAdmin(user('mk@getoya.ai')), false);
    assert.equal(isAdmin(user('ops@example.com', false)), false);
  });

  it('with OYA_ADMIN_REQUIRE_MFA on, refuses an admin signed in without a second factor', () => {
    process.env.OYA_ADMIN_REQUIRE_MFA = 'true';
    const admin = user('mk@getoya.ai');
    const refused = gate({ user: admin, authAal: 'aal1' });
    assert.deepEqual([refused.passed, refused.status, refused.body.code], [false, 403, 'mfa_required']);
    assert.equal(gate({ user: admin }).passed, false);
    assert.equal(gate({ user: admin, authAal: 'aal2' }).passed, true);
  });

  it('does not ask for a second factor when OYA_ADMIN_REQUIRE_MFA is off', () => {
    delete process.env.OYA_ADMIN_REQUIRE_MFA;
    assert.equal(gate({ user: user('mk@getoya.ai'), authAal: 'aal1' }).passed, true);
  });
});
