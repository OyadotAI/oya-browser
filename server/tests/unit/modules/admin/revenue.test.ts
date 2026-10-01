/**
 * Unit tests for revenue as the admin page reads it from Stripe: payments by
 * day, monthly recurring revenue, and what it shows without Stripe or when
 * Stripe fails.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { revenue } from '../../../../src/modules/admin/revenue.ts';

/** A Stripe that answers `bodies` by path, or throws. */
const fakeStripe = (bodies: Record<string, unknown>, fail = false) => ({
  post: async () => ({}),
  get: async (path: string) => {
    if (fail) throw new Error('Stripe: down');
    return bodies[path];
  },
});

/** 15 March 2026, midday UTC, in Stripe's seconds. */
const PAID_AT = Date.UTC(2026, 2, 15, 12) / 1000;

describe('admin revenue', () => {
  afterEach(() => delete process.env.STRIPE_SECRET_KEY);

  it('is off where Stripe is not set up, and never calls it', async () => {
    const r = await revenue(0, fakeStripe({}, true));
    assert.deepEqual(r, { enabled: false, mrrCents: 0, payments: [], error: '' });
  });

  it('answers paid invoices by the day they were paid, and MRR from fixed prices only', async () => {
    process.env.STRIPE_SECRET_KEY = 'sk_test';
    const items = [
      { price: { unit_amount: 2000, recurring: { usage_type: 'licensed', interval: 'month' } }, quantity: 1 },
      { price: { unit_amount: 12000, recurring: { usage_type: 'licensed', interval: 'year' } }, quantity: 1 },
      { price: { unit_amount: 5, recurring: { usage_type: 'metered', interval: 'month' } } },
    ];
    const r = await revenue(
      0,
      fakeStripe({
        '/invoices': { data: [{ amount_paid: 9900, created: 0, status_transitions: { paid_at: PAID_AT } }] },
        '/subscriptions': { data: [{ items: { data: items } }] },
      }),
    );
    assert.deepEqual(r.payments, [{ day: '2026-03-15', cents: 9900 }]);
    assert.equal(r.mrrCents, 3000);
  });

  it('says what Stripe said when it fails, rather than failing the page', async () => {
    process.env.STRIPE_SECRET_KEY = 'sk_test';
    const r = await revenue(0, fakeStripe({}, true));
    assert.equal(r.error, 'Stripe: down');
    assert.deepEqual(r.payments, []);
  });
});
