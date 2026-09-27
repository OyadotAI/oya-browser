/**
 * Unit tests for billing's storage: subscription rows, and a person's usage
 * for a period summed from their own hourly rows.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-billing-repo-');
const repo = await import('../../../../src/modules/billing/repository.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');
const usage = await import('../../../../src/platform/usage.ts');

describe('billing repository', () => {
  it('keeps what Stripe says and what was reported apart, so neither write undoes the other', async () => {
    await repo.saveStripe('u1', { plan: 'developer', status: 'active' });
    await repo.saveReported('u1', { period: 'p', oya_agent_steps: 3 });
    await repo.saveStripe('u1', { plan: 'free', status: 'canceled' });
    const row = await repo.find('u1');
    assert.equal(row?.plan, 'free');
    assert.deepEqual(row?.reported, { period: 'p', oya_agent_steps: 3 });
    assert.equal(await repo.find('u2'), null);
    assert.equal((await repo.all()).length, 1);
  });

  it('counts a period from the start of its first hour', () => {
    assert.equal(repo.hourStart('2026-03-10T14:23:11.000Z'), '2026-03-10T14:00:00.000Z');
  });

  it('sums a person’s billed usage from the period’s start, leaving earlier hours and other rows out', async () => {
    const row = (api_key: string, hour: string, agent_steps: number) => ({
      api_key,
      hour,
      agent_steps,
      cloud_seconds: 60,
    });
    await getConnection().upsert('usage', [
      row('u:u1', '2026-02-28T23:00:00.000Z', 100),
      row('u:u1', '2026-03-01T00:00:00.000Z', 3),
      row('u:u1', '2026-03-02T05:00:00.000Z', 4),
      row('fingerprint', '2026-03-02T05:00:00.000Z', 50),
    ]);
    const used = await repo.usageSince('u1', '2026-03-01T00:00:00.000Z');
    assert.equal(used.agent_steps, 7);
    assert.equal(used.cloud_seconds, 120);
    assert.equal(used.hosted_llm_microusd, 0);
    assert.equal(
      (await repo.usageSince('u1', '2026-02-28T23:30:00.000Z', '2026-03-02T00:00:00.000Z')).agent_steps,
      103,
    );
  });

  it('counts what was used this hour and not written yet', async () => {
    usage.reset();
    usage.billTo('key-live', 'u9');
    usage.record('key-live', 'agent_steps', 4);
    assert.equal((await repo.usageSince('u9', '2020-01-01T00:00:00.000Z')).agent_steps, 4);
    assert.equal((await repo.usageSince('u9', '2020-01-01T00:00:00.000Z', '2020-02-01T00:00:00.000Z')).agent_steps, 0);
  });
});
