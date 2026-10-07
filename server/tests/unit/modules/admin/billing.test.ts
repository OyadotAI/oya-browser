/** Admin billing changes preserve payment state and reject unsafe or duplicate grants. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-admin-billing-');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');
const { setPlan, grant } = await import('../../../../src/modules/admin/billing.ts');
const repo = await import('../../../../src/modules/billing/repository.ts');
const { creditsFor, standingOf, overrideFor } = await import('../../../../src/modules/billing/index.ts');
const NOW = Date.UTC(2026, 9, 7);
const SINCE = '2026-10-01T00:00:00.000Z';
await getConnection().upsert('profiles', [{ id: 'customer', email: 'customer@example.com' }]);

/** A valid support grant with its own id. */
const request = (more = {}) => ({ requestId: randomUUID(), hours: 2, credits: 3.5, reason: 'Support credit', ...more });

describe('admin billing', () => {
  it('grants access without creating a Stripe subscription and restores Free on removal', async () => {
    await setPlan('customer', { plan: 'startup', reason: 'Trial' }, 'admin');
    assert.equal(standingOf('customer', await repo.find('customer'), NOW).plan, 'startup');
    assert.equal((await repo.all()).length, 0);
    await setPlan('customer', { plan: null, reason: 'Trial ended' }, 'admin');
    assert.equal(standingOf('customer', await repo.find('customer'), NOW).plan, 'free');
    assert.equal((await overrideFor('customer'))?.actor, 'admin');
  });

  it('keeps overrides through Stripe updates and restores the latest subscription on removal', async () => {
    await repo.saveStripe('customer', {
      plan: 'developer',
      status: 'active',
      period_start: SINCE,
      stripe_customer_id: 'cus_1',
    });
    await setPlan('customer', { plan: 'free', reason: 'Restricted trial' }, 'admin');
    await repo.saveStripe('customer', { plan: 'startup', status: 'active' });
    assert.equal(standingOf('customer', await repo.find('customer'), NOW).plan, 'free');
    assert.equal((await repo.find('customer'))?.stripe_customer_id, 'cus_1');
    await setPlan('customer', { plan: null, reason: 'Restore' }, 'admin');
    assert.equal(standingOf('customer', await repo.find('customer'), NOW).plan, 'startup');
  });

  it('rejects unknown plans, missing reasons and nonexistent targets', async () => {
    for (const plan of ['enterprise', '__proto__', 1, undefined])
      await assert.rejects(setPlan('customer', { plan, reason: 'Test' }, 'admin'), { status: 400 });
    for (const reason of ['', ' ', 'x'.repeat(501), null])
      await assert.rejects(setPlan('customer', { plan: 'free', reason }, 'admin'), { status: 400 });
    await assert.rejects(setPlan('missing', { plan: 'free', reason: 'Test' }, 'admin'), { status: 404 });
    await assert.rejects(grant('missing', request(), 'admin', NOW), { status: 404 });
  });

  it('adds fractional hours and dollars once, retaining the actor and reason', async () => {
    const body = request({ hours: 0.25, credits: 1.25 });
    const first = await grant('customer', body, 'admin', NOW);
    assert.deepEqual(await grant('customer', body, 'admin', NOW), first);
    assert.equal(first.cloud_seconds, 900);
    assert.equal(first.hosted_llm_microusd, 1_250_000);
    assert.equal(first.actor, 'admin');
    assert.equal(first.reason, 'Support credit');
    assert.deepEqual(await creditsFor('customer', SINCE), { cloud_seconds: 900, hosted_llm_microusd: 1_250_000 });
    assert.deepEqual(await creditsFor('customer', '2026-11-01T00:00:00.000Z'), {
      cloud_seconds: 0,
      hosted_llm_microusd: 0,
    });
    await assert.rejects(grant('customer', { ...body, hours: 1 }, 'admin', NOW), { status: 400 });
    await assert.rejects(grant('customer', body, 'other-admin', NOW), { status: 400 });
  });

  it('does not lose simultaneous distinct grants or double a repeated request', async () => {
    const body = request({ hours: 1, credits: 0 });
    await Promise.all([
      grant('customer', body, 'admin', NOW),
      grant('customer', body, 'admin', NOW),
      grant('customer', request({ hours: 1, credits: 0 }), 'admin', NOW),
    ]);
    assert.equal((await creditsFor('customer', SINCE)).cloud_seconds, 8100);
  });

  it('rejects negative, nonfinite, nonnumeric, zero and excessive amounts', async () => {
    for (const hours of [-1, Infinity, NaN, '2', 100_001, null])
      await assert.rejects(grant('customer', request({ hours }), 'admin', NOW), { status: 400 });
    for (const credits of [-1, Infinity, '2', 100_001])
      await assert.rejects(grant('customer', request({ credits }), 'admin', NOW), { status: 400 });
    await assert.rejects(grant('customer', request({ hours: 0, credits: 0 }), 'admin', NOW), { status: 400 });
    await assert.rejects(grant('customer', request({ requestId: 'bad' }), 'admin', NOW), { status: 400 });
  });
});
