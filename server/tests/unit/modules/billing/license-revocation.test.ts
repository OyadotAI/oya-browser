/**
 * Unit tests for taking back self-hosted licenses: a full refund or a dispute
 * revokes the keys its subscription issued; a partial refund, or a payment for
 * anything else, leaves them.
 */
import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { revokeForEvent } from '../../../../src/modules/billing/license-revocation.ts';

/** 20 March 2026, midday UTC. */
const NOW = Date.UTC(2026, 2, 20, 12);

/** Revocation over one bought license for invoice in_L on subscription sub_L, and a Stripe that maps pi_L to it. */
function fixture() {
  const revoked: [string, string][] = [];
  const asked: any[] = [];
  const posts: any[] = [];
  const invoices: Record<string, any> = {
    in_early: { parent: { subscription_details: { metadata: { kind: 'self_hosted_license' } } } },
    in_plan: { parent: { subscription_details: { metadata: { user_id: 'u1' } } } },
  };
  const paidBy: Record<string, string> = { pi_L: 'in_L', pi_early: 'in_early', pi_plan: 'in_plan' };
  const deps = {
    stripe: {
      post: async (path, params) => (posts.push({ path, params }), {}),
      get: async (path, query) => {
        asked.push({ path, query });
        if (path.startsWith('/invoices/')) return invoices[path.slice('/invoices/'.length)];
        const invoice = paidBy[query.payment.payment_intent];
        return { data: invoice ? [{ invoice }] : [] };
      },
    },
    byInvoice: async (id) => (id === 'in_L' ? { id: 'L1', stripe_subscription_id: 'sub_L' } : null),
    revokeUnder: async (sub, at) => (revoked.push([sub, at]), 1),
    now: () => NOW,
  };
  return { deps, revoked, asked, posts };
}

/** An event of `type` on `object`. */
const on = (type: string, object: object) => ({ type, data: { object } });

describe('revokeForEvent', () => {
  beforeEach(() => mock.method(console, 'warn', () => {}));

  it('revokes the subscription’s keys when the license payment is refunded in full, and ends its billing', async () => {
    const { deps, revoked, asked, posts } = fixture();
    await revokeForEvent(on('charge.refunded', { refunded: true, payment_intent: 'pi_L' }), deps as any);
    assert.deepEqual(revoked, [['sub_L', new Date(NOW).toISOString()]]);
    assert.deepEqual(asked[0], {
      path: '/invoice_payments',
      query: { payment: { type: 'payment_intent', payment_intent: 'pi_L' }, limit: 1 },
    });
    assert.deepEqual(posts, [{ path: '/subscriptions/sub_L', params: { cancel_at_period_end: true } }]);
  });

  it('keeps the revocation when Stripe will not end the subscription', async () => {
    const { deps, revoked } = fixture();
    deps.stripe.post = async () => Promise.reject(new Error('Stripe: down'));
    await revokeForEvent(on('charge.dispute.created', { payment_intent: 'pi_L' }), deps as any);
    assert.equal(revoked.length, 1);
  });

  it('asks Stripe to retry a license refund that arrives before its key is recorded', async () => {
    const { deps, revoked } = fixture();
    await assert.rejects(
      revokeForEvent(on('charge.refunded', { refunded: true, payment_intent: 'pi_early' }), deps as any),
      {
        status: 503,
      },
    );
    assert.equal(revoked.length, 0);
  });

  it('revokes them when the license payment is disputed', async () => {
    const { deps, revoked } = fixture();
    await revokeForEvent(on('charge.dispute.created', { payment_intent: { id: 'pi_L' } }), deps as any);
    assert.equal(revoked.length, 1);
  });

  it('keeps the key on a partial refund', async () => {
    const { deps, revoked } = fixture();
    await revokeForEvent(on('charge.refunded', { refunded: false, payment_intent: 'pi_L' }), deps as any);
    assert.equal(revoked.length, 0);
  });

  it('leaves a refund of anything but a license alone', async () => {
    const { deps, revoked } = fixture();
    await revokeForEvent(on('charge.refunded', { refunded: true, payment_intent: 'pi_plan' }), deps as any);
    await revokeForEvent(on('charge.refunded', { refunded: true }), deps as any);
    assert.equal(revoked.length, 0);
  });

  it('reads the invoice off a charge from before API 2025-03-31 without asking Stripe', async () => {
    const { deps, revoked, asked } = fixture();
    await revokeForEvent(on('charge.dispute.created', { invoice: 'in_L' }), deps as any);
    assert.equal(revoked.length, 1);
    assert.equal(asked.length, 0);
  });

  it('ignores every other event', async () => {
    const { deps, revoked } = fixture();
    await revokeForEvent(on('customer.subscription.updated', { id: 'sub_L' }), deps as any);
    await revokeForEvent({ type: 'charge.refunded' }, deps as any);
    assert.equal(revoked.length, 0);
  });
});
