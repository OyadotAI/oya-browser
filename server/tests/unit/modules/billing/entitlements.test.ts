/**
 * Unit tests for admission against a person's plan: cloud time and how many at
 * once (including starts still on their way), agent steps, the hosted model,
 * the proxy rule, and the self-hosted license cap unless the license says this
 * is the hosted deployment.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { Entitlements } from '../../../../src/modules/billing/entitlements.ts';
import { PLANS, RESERVATION_MS } from '../../../../src/modules/billing/constants.ts';

/** 15 March 2026, midday UTC. */
const NOW = Date.UTC(2026, 2, 15, 12);

/** Admission over fakes: key "k-<user>" belongs to <user>; any other key belongs to nobody. */
function fixture({
  rows = {} as Record<string, any>,
  used = {} as Record<string, number>,
  cloud = [] as string[],
  hostedDeployment = true,
} = {}) {
  const billed: string[] = [];
  const license: number[] = [];
  const ownModel = new Set<string>();
  const clock = { now: NOW };
  const model = { name: 'gpt-4o-mini' };
  const deps = {
    find: async (userId) => rows[userId] ?? null,
    usageSince: async () => used,
    ownerOf: async (key) => (key.startsWith('k-') ? key.split('-')[1] : null),
    keyDigestsOf: async (userId) => new Set([`d:k-${userId}`, `d:k-${userId}-2`]),
    keyDigest: (key) => `d:${key}`,
    cloudKeys: () => cloud,
    billTo: (key, userId) => billed.push(`${key}>${userId}`),
    ownsModel: (key) => ownModel.has(key),
    modelOf: () => model.name,
    licenseAdmit: (count) => (license.push(count), { ok: count <= 5, message: 'needs a license' }),
    hostedDeployment: () => hostedDeployment,
    upgradeUrl: () => 'https://console.test/dashboard/billing',
    now: () => clock.now,
  };
  return { e: new Entitlements(deps), billed, license, ownModel, clock, model };
}

/** An active paid subscription for `plan`. */
const paid = (plan: string, status = 'active') => ({
  user_id: 'u',
  plan,
  status,
  period_start: '2026-03-10T00:00:00.000Z',
});

describe('Entitlements', () => {
  beforeEach(() => (process.env.STRIPE_SECRET_KEY = 'sk_test'));
  afterEach(() => delete process.env.STRIPE_SECRET_KEY);

  it('admits a Free person’s cloud browser while their hour lasts, and books their usage to them', async () => {
    const { e, billed } = fixture({ used: { cloud_seconds: 3599 } });
    await e.admitCloud('k-u');
    assert.deepEqual(billed, ['k-u>u']);
  });

  it('refuses a Free person’s cloud browser once the hour is used, with where to upgrade', async () => {
    const { e } = fixture({ used: { cloud_seconds: PLANS.free.cloudSeconds } });
    await assert.rejects(e.admitCloud('k-u'), (err: any) => {
      assert.equal(err.status, 402);
      assert.equal(err.code, 'plan_limit');
      assert.equal(err.upgrade_url, 'https://console.test/dashboard/billing');
      assert.match(err.message, /sales@getoya\.ai/);
      return true;
    });
  });

  it('lets a paid plan run past its included hours, as overage', async () => {
    const { e } = fixture({
      rows: { u: paid('developer') },
      used: { cloud_seconds: 10 * PLANS.developer.cloudSeconds },
    });
    await e.admitCloud('k-u');
  });

  it('counts cloud browsers at once across all of a person’s keys', async () => {
    const { e } = fixture({ cloud: ['k-u', 'k-u-2', 'k-other'] });
    await e.admitCloud('k-u');
    await assert.rejects(e.admitCloud('k-u'), { code: 'plan_concurrency' });
  });

  it('counts starts still on their way, so a burst of starts cannot pass the cap together', async () => {
    const { e } = fixture();
    await e.admitCloud('k-u', 2);
    await assert.rejects(e.admitCloud('k-u', 2), { code: 'plan_concurrency' });
  });

  it('frees a held place when the browser connects, or after a while', async () => {
    const { e, clock } = fixture();
    await e.admitCloud('k-u', 3);
    await e.attribute('k-u');
    await e.admitCloud('k-u');
    await assert.rejects(e.admitCloud('k-u'), { code: 'plan_concurrency' });
    clock.now += RESERVATION_MS + 1;
    await e.admitCloud('k-u', 3);
  });

  it('refuses a person whose last payment failed', async () => {
    const { e } = fixture({ rows: { u: paid('developer', 'past_due') } });
    await assert.rejects(e.admitCloud('k-u'), { status: 402, code: 'payment_required' });
  });

  it('never holds a key nobody owns to a plan (env keys, the fleet token)', async () => {
    const { e, billed } = fixture({ used: { cloud_seconds: 1e9 } });
    await e.admitCloud('env-key');
    await e.admitAgent('env-key');
    assert.deepEqual(billed, []);
  });

  it('counts agent steps whoever’s model runs them: a Free person’s 501st step is refused', async () => {
    const { e, ownModel } = fixture({ used: { agent_steps: PLANS.free.steps } });
    ownModel.add('k-u');
    await assert.rejects(e.admitAgent('k-u'), { status: 402, code: 'plan_limit' });
  });

  it('holds a Free person on the hosted model to its allowance, but not one with their own model', async () => {
    const { e, ownModel } = fixture({ used: { agent_steps: 1, hosted_llm_microusd: PLANS.free.llmMicroUsd } });
    await assert.rejects(e.admitAgent('k-u'), { code: 'plan_limit' });
    ownModel.add('k-u');
    await e.admitAgent('k-u');
  });

  it('runs the hosted model only on models it prices, whatever the plan', async () => {
    const { e, model, ownModel } = fixture({ rows: { u: paid('startup') } });
    model.name = 'o1-pro';
    await assert.rejects(e.admitAgent('k-u'), { status: 400, code: 'model_not_hosted' });
    ownModel.add('k-u');
    await e.admitAgent('k-u');
  });

  it('gives the residential proxy only to keys it knows are on a paid plan', async () => {
    const { e } = fixture({ rows: { v: paid('startup') } });
    assert.equal(e.proxyAllowed('k-v'), false, 'unknown until the owner is looked up');
    await e.admitCloud('k-u');
    await e.admitCloud('k-v');
    assert.equal(e.proxyAllowed('k-u'), false);
    assert.equal(e.proxyAllowed('k-v'), true);
  });

  it('learns a reconnecting browser’s owner before it is welcomed', async () => {
    const { e, billed } = fixture({ rows: { u: paid('developer') } });
    await e.attribute('k-u');
    assert.deepEqual(billed, ['k-u>u']);
    assert.equal(e.proxyAllowed('k-u'), true);
  });

  it('attributes a late payer without throwing, and without the proxy', async () => {
    const { e } = fixture({ rows: { u: paid('developer', 'past_due') } });
    await e.attribute('k-u');
    assert.equal(e.proxyAllowed('k-u'), false);
  });

  it('holds a server that is not the licensed hosted deployment to the cap, Stripe key or not', async () => {
    const { e } = fixture({ cloud: ['a', 'b', 'c', 'd', 'e'], hostedDeployment: false });
    await assert.rejects(e.admitCloud('env-key'), { code: 'license_required' });
  });

  describe('self-hosted, with no plans', () => {
    beforeEach(() => delete process.env.STRIPE_SECRET_KEY);

    it('asks the license about every cloud browser on the server plus the new ones', async () => {
      const { e, license } = fixture({ cloud: ['a', 'b', 'c', 'd'], hostedDeployment: false });
      await e.admitCloud('k-u');
      assert.deepEqual(license, [5]);
    });

    it('refuses the sixth cloud browser without a license, naming who to write to', async () => {
      const { e } = fixture({ cloud: ['a', 'b', 'c', 'd', 'e'], hostedDeployment: false });
      await assert.rejects(e.admitCloud('k-u'), { status: 402, code: 'license_required', contact: 'sales@getoya.ai' });
    });

    it('counts starts on their way against the cap too', async () => {
      const { e } = fixture({ hostedDeployment: false });
      await e.admitCloud('k-u', 5);
      await assert.rejects(e.admitCloud('k-u'), { code: 'license_required' });
    });

    it('has no plans to hold agents or proxies to', async () => {
      const { e } = fixture({ used: { agent_steps: 1e9 }, hostedDeployment: false });
      await e.admitAgent('k-u');
      assert.equal(e.proxyAllowed('k-u'), true);
      await e.attribute('k-u');
    });
  });
});
