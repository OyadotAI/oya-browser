/**
 * Unit tests for billing's routes: the plan summary, Checkout and the Portal
 * for a signed-in person, and the Stripe webhook that only Stripe can call.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { createHmac } from 'node:crypto';
import { billingRoutes, billingWebhook } from '../../../../src/modules/billing/routes.ts';
import { FakeResponse } from '../../support/auth.ts';

/** Billing with each part recording what it was asked. */
function fakeBilling() {
  const asked: any[] = [];
  const subscriptions = {
    summary: async (id) => (asked.push(['summary', id]), { plan: 'free' }),
    checkout: async (id, plan) => (asked.push(['checkout', id, plan]), { url: 'c' }),
    portal: async (id) => (asked.push(['portal', id]), { url: 'p' }),
    applyEvent: async (e) => void asked.push(['event', e.type]),
  };
  const licenses = {
    view: async (id) => (asked.push(['license', id]), { enabled: true, license: null }),
    checkout: async (id, n, interval) => (asked.push(['license-checkout', id, n, interval]), { url: 'lc' }),
    portal: async (id) => (asked.push(['license-portal', id]), { url: 'lp' }),
    applyEvent: async (e) => void asked.push(['license-event', e.type]),
  };
  return { billing: { subscriptions, licenses } as any, asked };
}

/** The handler behind a route, past its sign-in middleware. */
function handler(router: any, method: string, path: string) {
  const layer = router.stack.find((l) => l.route?.path === path && l.route.methods[method]);
  return layer.route.stack.at(-1).handle;
}

/** Calls a route's handler as a signed-in person. */
async function call(billing: any, method: string, path: string, body = {}) {
  const res = new FakeResponse();
  await handler(billingRoutes(billing), method, path)({ user: { id: 'u1' }, body }, res);
  return res;
}

/** A webhook request carrying `event`, signed with `secret`. */
function webhook(event: object, secret = 'whsec_test') {
  const raw = Buffer.from(JSON.stringify(event));
  const t = Math.floor(Date.now() / 1000);
  const sig = createHmac('sha256', secret).update(`${t}.`).update(raw).digest('hex');
  return { body: raw, headers: { 'stripe-signature': `t=${t},v1=${sig}` } };
}

describe('billing routes', () => {
  beforeEach(() => Object.assign(process.env, { STRIPE_SECRET_KEY: 'sk_test', STRIPE_WEBHOOK_SECRET: 'whsec_test' }));
  afterEach(() => ['STRIPE_SECRET_KEY', 'STRIPE_WEBHOOK_SECRET'].forEach((n) => delete process.env[n]));

  it('answers the signed-in person’s plan', async () => {
    const { billing, asked } = fakeBilling();
    assert.deepEqual((await call(billing, 'get', '/billing')).body, { plan: 'free' });
    assert.deepEqual(asked, [['summary', 'u1']]);
  });

  it('says a self-hosted server has no plans', async () => {
    delete process.env.STRIPE_SECRET_KEY;
    const { billing } = fakeBilling();
    assert.deepEqual((await call(billing, 'get', '/billing')).body, { enabled: false });
  });

  it('opens Checkout for the plan asked for, and the Portal', async () => {
    const { billing, asked } = fakeBilling();
    assert.deepEqual((await call(billing, 'post', '/billing/checkout', { plan: 'startup' })).body, { url: 'c' });
    assert.deepEqual((await call(billing, 'post', '/billing/portal')).body, { url: 'p' });
    assert.deepEqual(asked, [
      ['checkout', 'u1', 'startup'],
      ['portal', 'u1'],
    ]);
  });

  it('answers the person’s self-hosted license, opens its Checkout with the count and period asked, and its Portal', async () => {
    const { billing, asked } = fakeBilling();
    assert.deepEqual((await call(billing, 'get', '/billing/license')).body, { enabled: true, license: null });
    const body = { browsers: 12, interval: 'year' };
    assert.deepEqual((await call(billing, 'post', '/billing/license/checkout', body)).body, { url: 'lc' });
    assert.deepEqual((await call(billing, 'post', '/billing/license/portal')).body, { url: 'lp' });
    assert.deepEqual(asked, [
      ['license', 'u1'],
      ['license-checkout', 'u1', 12, 'year'],
      ['license-portal', 'u1'],
    ]);
  });

  it('says a self-hosted server sells no licenses itself', async () => {
    delete process.env.STRIPE_SECRET_KEY;
    const { billing } = fakeBilling();
    assert.deepEqual((await call(billing, 'get', '/billing/license')).body, { enabled: false });
  });

  it('applies an event Stripe signed', async () => {
    const { billing, asked } = fakeBilling();
    const res = new FakeResponse();
    await billingWebhook(billing)(webhook({ type: 'customer.subscription.updated' }), res);
    assert.deepEqual(res.body, { received: true });
    assert.deepEqual(asked, [
      ['event', 'customer.subscription.updated'],
      ['license-event', 'customer.subscription.updated'],
    ]);
  });

  it('refuses an event anyone else signed, so nobody can grant themselves a plan', async () => {
    const { billing, asked } = fakeBilling();
    const res = new FakeResponse();
    await billingWebhook(billing)(webhook({ type: 'customer.subscription.created' }, 'whsec_forged'), res);
    assert.equal(res.statusCode, 400);
    assert.deepEqual(asked, []);
  });

  it('takes no webhook on a self-hosted server, or with a missing or non-JSON body', async () => {
    const { billing, asked } = fakeBilling();
    const missing = new FakeResponse();
    await billingWebhook(billing)({ body: undefined, headers: { 'stripe-signature': 't=1,v1=deadbeef' } }, missing);
    assert.equal(missing.statusCode, 400);
    const badJson = new FakeResponse();
    const raw = Buffer.from('not-json');
    const t = Math.floor(Date.now() / 1000);
    const sig = createHmac('sha256', 'whsec_test').update(`${t}.`).update(raw).digest('hex');
    await billingWebhook(billing)({ body: raw, headers: { 'stripe-signature': `t=${t},v1=${sig}` } }, badJson);
    assert.equal(badJson.statusCode, 400);
    delete process.env.STRIPE_SECRET_KEY;
    const res = new FakeResponse();
    await billingWebhook(billing)(webhook({ type: 'customer.subscription.created' }), res);
    assert.equal(res.statusCode, 400);
    assert.deepEqual(asked, []);
  });
});
