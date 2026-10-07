/**
 * Unit tests for subscribing through Stripe and keeping a plan in step with
 * Stripe's webhooks.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { Subscriptions } from '../../../../src/modules/billing/subscriptions.ts';
import { PLANS } from '../../../../src/modules/billing/constants.ts';

/** 15 March 2026, midday UTC. */
const NOW = Date.UTC(2026, 2, 15, 12);
/** Seconds for 10 March 2026 and 10 April 2026. */
const [START, END] = [Date.UTC(2026, 2, 10) / 1000, Date.UTC(2026, 3, 10) / 1000];

/** Subscriptions over a table in memory and a Stripe that records its calls. */
function fixture() {
  const rows = new Map<string, any>();
  const posts: any[] = [];
  const stripe = {
    post: async (path, params) => (posts.push({ path, params }), { url: `https://stripe.test${path}` }),
  };
  const deps = {
    find: async (id) => rows.get(id) ?? null,
    saveStripe: async (id, cols) => void rows.set(id, { ...rows.get(id), user_id: id, ...cols }),
    usageSince: async () => ({ cloud_seconds: 60 }),
    stripe,
    returnUrl: () => 'https://console.test/dashboard/billing',
    now: () => NOW,
  };
  return { s: new Subscriptions(deps), rows, posts };
}

/** A Stripe subscription event for user u1 on `price`. */
const event = (type: string, price = 'base_dev', status = 'active', sub = 'sub_1', created = 100) => ({
  type,
  created,
  data: {
    object: {
      id: sub,
      customer: 'cus_1',
      status,
      metadata: { user_id: 'u1' },
      items: {
        data: [
          { price: { id: 'min_dev' } },
          { price: { id: price }, current_period_start: START, current_period_end: END },
        ],
      },
    },
  },
});

describe('Subscriptions', () => {
  beforeEach(() => (process.env.STRIPE_PRICES_DEVELOPER = 'base_dev,min_dev,mb_dev,llm_dev,steps_dev'));
  afterEach(() => delete process.env.STRIPE_PRICES_DEVELOPER);

  it('opens Checkout for a plan: its base price once, its metered prices by usage, tagged with the person', async () => {
    const { s, posts } = fixture();
    assert.deepEqual(await s.checkout('u1', 'developer'), { url: 'https://stripe.test/checkout/sessions' });
    const { params } = posts[0];
    assert.deepEqual(params.line_items[0], { price: 'base_dev', quantity: 1 });
    assert.deepEqual(params.line_items[1], { price: 'min_dev' });
    assert.equal(params.subscription_data.metadata.user_id, 'u1');
    assert.equal(params.customer, undefined);
  });

  it('opens Checkout in Oya’s look: its name, icon and colors', async () => {
    const { s, posts } = fixture();
    await s.checkout('u1', 'developer');
    assert.equal(posts[0].params.branding_settings.display_name, 'Oya Browser');
    assert.match(posts[0].params.branding_settings.icon.url, /^https:\/\/oyabrowser\.com\//);
  });

  it('still opens Checkout, plain, if Stripe refuses the look', async () => {
    const posts: any[] = [];
    const stripe = {
      post: async (_p, params) => {
        posts.push(params);
        if (params.branding_settings) throw new Error('Stripe: Received unknown parameter: branding_settings');
        return { url: 'plain' };
      },
      get: async () => ({}),
    };
    const s = new Subscriptions({
      find: async () => null,
      saveStripe: async () => {},
      usageSince: async () => ({}),
      stripe,
      returnUrl: () => 'r',
      now: () => NOW,
    });
    assert.deepEqual(await s.checkout('u1', 'developer'), { url: 'plain' });
    assert.equal(posts.length, 2);
  });

  it('does not hide any other Checkout failure', async () => {
    const stripe = {
      post: async () => {
        throw new Error('Stripe: No such price');
      },
      get: async () => ({}),
    };
    const s = new Subscriptions({
      find: async () => null,
      saveStripe: async () => {},
      usageSince: async () => ({}),
      stripe,
      returnUrl: () => 'r',
      now: () => NOW,
    });
    await assert.rejects(s.checkout('u1', 'developer'), /No such price/);
  });

  it('says when the current period ends', async () => {
    const { s, rows } = fixture();
    rows.set('u1', {
      user_id: 'u1',
      plan: 'developer',
      status: 'active',
      period_start: '2026-03-10T00:00:00.000Z',
      period_end: '2026-04-10T00:00:00.000Z',
    });
    assert.equal((await s.summary('u1')).until, '2026-04-10T00:00:00.000Z');
  });

  it('reuses the person’s Stripe customer when they have one', async () => {
    const { s, rows, posts } = fixture();
    rows.set('u1', { user_id: 'u1', stripe_customer_id: 'cus_1' });
    await s.checkout('u1', 'developer');
    assert.equal(posts[0].params.customer, 'cus_1');
  });

  it('refuses a plan that is not for sale, and a plan with no prices yet', async () => {
    const { s } = fixture();
    await assert.rejects(s.checkout('u1', 'free'), { status: 400 });
    await assert.rejects(s.checkout('u1', 'startup'), { status: 503 });
  });

  it('opens the Customer Portal for a subscriber, and 404s for anyone else', async () => {
    const { s, rows } = fixture();
    await assert.rejects(s.portal('u1'), { status: 404 });
    rows.set('u1', { user_id: 'u1', stripe_customer_id: 'cus_1' });
    assert.deepEqual(await s.portal('u1'), { url: 'https://stripe.test/billing_portal/sessions' });
  });

  it('opens Oya’s own portal configuration where one is set, and the account default otherwise', async () => {
    const { s, rows, posts } = fixture();
    rows.set('u1', { user_id: 'u1', stripe_customer_id: 'cus_1' });
    await s.portal('u1');
    process.env.STRIPE_PORTAL_CONFIGURATION = 'bpc_oya';
    try {
      await s.portal('u1');
    } finally {
      delete process.env.STRIPE_PORTAL_CONFIGURATION;
    }
    assert.deepEqual(
      posts.map((p) => p.params.configuration),
      [undefined, 'bpc_oya'],
    );
  });

  it('keeps a subscription’s plan, status and period, read from its items', async () => {
    const { s, rows } = fixture();
    await s.applyEvent(event('customer.subscription.created'));
    const row = rows.get('u1');
    assert.equal(row.plan, 'developer');
    assert.equal(row.status, 'active');
    assert.equal(row.stripe_customer_id, 'cus_1');
    assert.equal(row.period_start, '2026-03-10T00:00:00.000Z');
    assert.equal(row.period_end, '2026-04-10T00:00:00.000Z');
  });

  it('keeps what was already reported when the subscription changes', async () => {
    const { s, rows } = fixture();
    rows.set('u1', { user_id: 'u1', reported: { period: 'p' } });
    await s.applyEvent(event('customer.subscription.updated', 'base_dev', 'past_due'));
    assert.deepEqual(rows.get('u1').reported, { period: 'p' });
    assert.equal(rows.get('u1').status, 'past_due');
  });

  it('puts a deleted subscription back on Free', async () => {
    const { s, rows } = fixture();
    await s.applyEvent(event('customer.subscription.deleted'));
    assert.equal(rows.get('u1').plan, 'free');
    assert.equal(rows.get('u1').status, 'canceled');
  });

  it('reads the period from the subscription itself on an older API version', async () => {
    const { s, rows } = fixture();
    const old = event('customer.subscription.updated');
    Object.assign(old.data.object, { items: { data: [] }, current_period_start: START });
    await s.applyEvent(old);
    assert.equal(rows.get('u1').plan, 'free');
    assert.equal(rows.get('u1').period_start, '2026-03-10T00:00:00.000Z');
  });

  it('ignores an older event about the same subscription, which Stripe may deliver late', async () => {
    const { s, rows } = fixture();
    await s.applyEvent(event('customer.subscription.updated', 'base_dev', 'active', 'sub_1', 200));
    await s.applyEvent(event('customer.subscription.created', 'base_dev', 'incomplete', 'sub_1', 150));
    assert.equal(rows.get('u1').status, 'active');
  });

  it('ignores the end of a subscription the person has since replaced', async () => {
    const { s, rows } = fixture();
    await s.applyEvent(event('customer.subscription.created', 'base_dev', 'active', 'sub_new', 300));
    await s.applyEvent(event('customer.subscription.deleted', 'base_dev', 'canceled', 'sub_old', 400));
    assert.equal(rows.get('u1').plan, 'developer');
    assert.equal(rows.get('u1').stripe_subscription_id, 'sub_new');
  });

  it('ignores events it does not follow, and subscriptions with no person on them', async () => {
    const { s, rows } = fixture();
    await s.applyEvent(event('invoice.paid'));
    const anonymous = event('customer.subscription.created');
    anonymous.data.object.metadata = {} as any;
    await s.applyEvent(anonymous);
    await s.applyEvent({ type: 'customer.subscription.created' });
    assert.equal(rows.size, 0);
  });

  it('names a paying person’s plan, and Free for anyone else', async () => {
    const { s, rows } = fixture();
    rows.set('u1', { user_id: 'u1', plan: 'developer', status: 'active', period_start: '2026-03-10T00:00:00.000Z' });
    assert.deepEqual([await s.plan('u1'), await s.plan('u2')], ['developer', 'free']);
  });

  it('summarizes a person’s plan, allowances and use this period', async () => {
    const { s } = fixture();
    const summary = await s.summary('u1');
    assert.equal(summary.plan, 'free');
    assert.deepEqual(summary.included, PLANS.free);
    assert.deepEqual(summary.used, { cloud_seconds: 60 });
    assert.equal(summary.since, '2026-03-01T00:00:00.000Z');
  });
});

it('shows actual usage alongside the extended allowance in the customer billing summary', async () => {
  const { s } = fixture();
  s.deps.creditsFor = async () => ({ cloud_seconds: 3600, hosted_llm_microusd: 1_000_000 });
  const summary = await s.summary('u');
  assert.equal(summary.used.cloud_seconds, 60);
  assert.equal(summary.included.cloudSeconds, PLANS.free.cloudSeconds + 3600);
  assert.equal(summary.included.llmMicroUsd, PLANS.free.llmMicroUsd + 1_000_000);
});
