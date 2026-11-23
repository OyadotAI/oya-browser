/**
 * Unit tests for where bought self-hosted licenses are kept: found by their
 * invoice, listed for their buyer newest first, revoked ones left out.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-license-repo-');
const repo = await import('../../../../src/modules/billing/license-repository.ts');

/** A bought license row. */
const row = (id: string, user: string, at: string, revoked: string | null = null) => ({
  ...{ id, licensee: 'ops@acme.test', max_concurrent: 6, expires_at: '2027-01-01T00:00:00.000Z' },
  ...{ created_at: at, created_by: 'stripe', revoked_at: revoked, user_id: user, stripe_invoice_id: `in_${id}` },
  stripe_subscription_id: `sub_${user}`,
});

describe('license repository', () => {
  it('finds a license by its invoice, and nothing for an invoice never paid', async () => {
    await repo.insert(row('L1', 'u1', '2026-11-01T00:00:00.000Z'));
    assert.equal((await repo.byInvoice('in_L1'))?.id, 'L1');
    assert.equal(await repo.byInvoice('in_none'), null);
  });

  it('lists a buyer’s live licenses newest first, without revoked ones or anyone else’s', async () => {
    await repo.insert(row('L2', 'u1', '2026-12-01T00:00:00.000Z'));
    await repo.insert(row('L3', 'u1', '2026-12-15T00:00:00.000Z', '2026-12-16T00:00:00.000Z'));
    await repo.insert(row('L4', 'u2', '2026-12-20T00:00:00.000Z'));
    assert.deepEqual(
      (await repo.forUser('u1')).map((r) => r.id),
      ['L2', 'L1'],
    );
  });

  it('revokes the live licenses under a subscription, and then says it has a revoked one', async () => {
    assert.equal(await repo.revokedUnder('sub_u2'), false);
    assert.equal(await repo.revokeUnder('sub_u2', '2026-12-21T00:00:00.000Z'), 1);
    assert.equal(await repo.revokedUnder('sub_u2'), true);
    assert.deepEqual(await repo.forUser('u2'), []);
  });
});
