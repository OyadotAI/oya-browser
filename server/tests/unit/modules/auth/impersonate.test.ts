/**
 * Unit tests for "Login as" tokens: a genuine one reads back, and a forged,
 * expired, reshaped or unsigned one never does; and every use re-checks that
 * its admin still is one.
 */
import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { Status } from '../../../../src/platform/http-status.ts';
import { restoreEnv } from '../../support/data-dir.ts';
import {
  actingAs,
  mintImpersonation,
  readImpersonation,
  impersonatedUser,
} from '../../../../src/modules/auth/impersonate.ts';

/** 15 March 2026, midday UTC. */
const NOW = Date.UTC(2026, 2, 15, 12);
/** Just past the hour a token lasts. */
const LATER = NOW + 3_600_001;
const saved = { own: process.env.OYA_IMPERSONATE_SECRET, service: process.env.SUPABASE_SERVICE_KEY };

/** The middle part of a token rewritten with these claims. */
function reshaped(token: string, claims: object) {
  const [head, , sig] = token.split('.');
  return `${head}.${Buffer.from(JSON.stringify(claims)).toString('base64url')}.${sig}`;
}

describe('Login as tokens', () => {
  before(() => {
    process.env.OYA_IMPERSONATE_SECRET = 'x'.repeat(32);
  });
  after(() => {
    restoreEnv('OYA_IMPERSONATE_SECRET', saved.own);
    restoreEnv('SUPABASE_SERVICE_KEY', saved.service);
  });

  it('reads back who is acted as and by which admin', () => {
    const claims = readImpersonation(mintImpersonation('cust', 'admin', NOW), NOW);
    assert.deepEqual([claims?.sub, claims?.impersonated_by, claims?.imp], ['cust', 'admin', true]);
  });

  it('expires after an hour', () => {
    assert.equal(readImpersonation(mintImpersonation('cust', 'admin', NOW), LATER), null);
  });

  it('refuses a token whose claims were changed after signing', () => {
    const token = mintImpersonation('cust', 'admin', NOW);
    assert.equal(
      readImpersonation(reshaped(token, { sub: 'other', impersonated_by: 'admin', imp: true, exp: 9e9 }), NOW),
      null,
    );
  });

  it('refuses a token signed with another secret', () => {
    const token = mintImpersonation('cust', 'admin', NOW);
    process.env.OYA_IMPERSONATE_SECRET = 'y'.repeat(32);
    assert.equal(readImpersonation(token, NOW), null);
    process.env.OYA_IMPERSONATE_SECRET = 'x'.repeat(32);
  });

  it('refuses garbage and an unsigned token', () => {
    const [head, payload] = mintImpersonation('cust', 'admin', NOW).split('.');
    for (const bad of ['', 'a.b.c', `${head}.${payload}.`, `${head}.${payload}`])
      assert.equal(readImpersonation(bad, NOW), null);
  });

  it('derives a secret from the service key when none is set', () => {
    delete process.env.OYA_IMPERSONATE_SECRET;
    process.env.SUPABASE_SERVICE_KEY = 'service-key';
    assert.ok(readImpersonation(mintImpersonation('cust', 'admin', NOW), NOW));
    process.env.OYA_IMPERSONATE_SECRET = 'x'.repeat(32);
  });

  it('refuses to mint with no secret, or one too short', () => {
    delete process.env.SUPABASE_SERVICE_KEY;
    process.env.OYA_IMPERSONATE_SECRET = 'short';
    assert.throws(() => mintImpersonation('cust', 'admin'), { status: Status.UNAVAILABLE });
    process.env.OYA_IMPERSONATE_SECRET = 'x'.repeat(32);
  });

  it('cannot look a user up without Supabase Auth', async () => {
    await assert.rejects(impersonatedUser('cust'), { status: Status.UNAVAILABLE });
  });
});

describe('actingAs', () => {
  /** A user lookup over these people, by id; anyone else is gone. */
  const people = (byId: Record<string, string>) => async (id: string) => {
    if (!byId[id]) throw Object.assign(new Error('gone'), { status: Status.NOT_FOUND });
    return { id, email: byId[id], email_confirmed_at: 'x' } as any;
  };
  const claims = { sub: 'cust', impersonated_by: 'adm', imp: true as const, iat: 0, exp: 0 };

  it('acts as the customer while the admin still is one', async () => {
    const find = people({ cust: 'c@example.com', adm: 'a@getoya.ai' });
    assert.equal((await actingAs(claims, find)).id, 'cust');
  });

  it('ends the token once its admin is no longer an admin, or is gone', async () => {
    for (const admin of ['a@example.com', '']) {
      const find = people({ cust: 'c@example.com', ...(admin ? { adm: admin } : {}) });
      await assert.rejects(actingAs(claims, find), { status: Status.UNAUTHORIZED });
    }
  });

  it('ends the token once the customer has become an admin', async () => {
    const find = people({ cust: 'c@getoya.ai', adm: 'a@getoya.ai' });
    await assert.rejects(actingAs(claims, find), { status: Status.UNAUTHORIZED });
  });
});
