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
  return { billing: { subscriptions } as any, asked };
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

  it('applies an event Stripe signed', async () => {
    const { billing, asked } = fakeBilling();
    const res = new FakeResponse();
    await billingWebhook(billing)(webhook({ type: 'customer.subscription.updated' }), res);
    assert.deepEqual(res.body, { received: true });
    assert.deepEqual(asked, [['event', 'customer.subscription.updated']]);
  });

  it('refuses an event anyone else signed, so nobody can grant themselves a plan', async () => {
    const { billing, asked } = fakeBilling();
    const res = new FakeResponse();
    await billingWebhook(billing)(webhook({ type: 'customer.subscription.created' }, 'whsec_forged'), res);
    assert.equal(res.statusCode, 400);
    assert.deepEqual(asked, []);
  });

  it('takes no webhook on a self-hosted server', async () => {
    delete process.env.STRIPE_SECRET_KEY;
    const { billing, asked } = fakeBilling();
    const res = new FakeResponse();
    await billingWebhook(billing)(webhook({ type: 'customer.subscription.created' }), res);
    assert.equal(res.statusCode, 400);
    assert.deepEqual(asked, []);
  });
});
