/**
 * Unit tests for who may open the admin page.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { adminOnly, isAdmin } from '../../../../src/modules/admin/access.ts';

/** A confirmed user with this email. */
const user = (email: string, confirmed = true) => ({
  email,
  email_confirmed_at: confirmed ? '2026-01-01T00:00:00Z' : null,
});

describe('admin access', () => {
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
});
