/** Financial reads are exact only after bounded full traversal and retain currency and payment-date scope. */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { revenue, mrr, payments } from '../../../../src/modules/admin/revenue.ts';

/** Midday March 16 excludes today's incomplete UTC interval. */
const NOW = Date.UTC(2026, 2, 16, 12);
/** Stripe's final payment timestamp, deliberately unrelated to creation time. */
const PAID = Date.UTC(2026, 2, 15, 12) / 1000;
/** A final collection page. */
const page = (data: any[], has_more = false) => ({ data, has_more });
/** One supported fixed item. */
const fixed = (id = 'item', extra: any = {}) => ({
  id,
  quantity: 1,
  price: {
    currency: 'usd',
    billing_scheme: 'per_unit',
    unit_amount: 1200,
    recurring: { usage_type: 'licensed', interval: 'month', interval_count: 1 },
    ...extra,
  },
});
/** One active subscription with complete embedded items. */
const subscription = (items: any[] = [fixed()]) => ({ id: 'sub', items: page(items) });
/** One paid invoice. */
const invoice = (id = 'inv', extra: any = {}) => ({
  id,
  created: 1,
  amount_paid: 9900,
  currency: 'usd',
  status_transitions: { paid_at: PAID },
  ...extra,
});
/** Fake Stripe records every query and signal without touching the network. */
function fake(read: (path: string, query: any, signal?: AbortSignal) => any) {
  const calls: any[] = [];
  return {
    calls,
    post: async () => ({}),
    get: async (path: string, query: any, signal?: AbortSignal) => {
      calls.push({ path, query, signal });
      return read(path, query, signal);
    },
  };
}
/** A complete MRR response over one subscription. */
const recurring = (items: any[]) => fake(() => page([subscription(items)]));

describe('admin financial snapshots', () => {
  afterEach(() => delete process.env.STRIPE_SECRET_KEY);

  it('reports not configured without calling Stripe or inventing zero', async () => {
    const stripe = fake(() => {
      throw new Error('must not call');
    });
    const result = await revenue(NOW, stripe);
    assert.deepEqual(
      [result.mrr.state, result.mrr.values, result.paidInvoices.state, result.paidInvoices.values],
      ['not_configured', null, 'not_configured', null],
    );
    assert.equal(stripe.calls.length, 0);
  });

  it('keeps invoices separate from MRR and filters by final paid date including old-created invoices', async () => {
    process.env.STRIPE_SECRET_KEY = 'sk_test';
    const stripe = fake((path) =>
      path === '/subscriptions'
        ? page([subscription()])
        : page([
            invoice(),
            invoice('today', { status_transitions: { paid_at: NOW / 1000 } }),
            invoice('old', { status_transitions: { paid_at: PAID - 40 * 86400 } }),
          ]),
    );
    const result = await revenue(NOW, stripe);
    assert.deepEqual(result.mrr.values, [{ currency: 'usd', amountMinor: 1200, minorUnitExponent: 2 }]);
    assert.deepEqual(result.paidInvoices.values, [{ currency: 'usd', amountMinor: 9900, minorUnitExponent: 2 }]);
    assert.equal(result.paidInvoices.days?.length, 30);
    assert.ok(
      stripe.calls.every(({ query, signal }) => !Object.hasOwn(query, 'created[gte]') && signal instanceof AbortSignal),
    );
  });

  it('traverses top-level pages and nested item continuations without counting embedded items twice', async () => {
    const stripe = fake((path, query) => {
      if (path === '/subscription_items') {
        assert.equal(query.starting_after, 'a');
        return page([fixed('b')]);
      }
      if (!query.starting_after) return page([{ id: 'first', items: page([fixed('a')], true) }], true);
      assert.equal(query.starting_after, 'first');
      return page([{ id: 'second', items: page([fixed('c')]) }]);
    });
    assert.deepEqual((await mrr(stripe)).values, [{ currency: 'usd', amountMinor: 3600, minorUnitExponent: 2 }]);
    assert.equal(stripe.calls.length, 3);
    assert.ok(stripe.calls.every(({ query }) => query.limit === 100));
  });

  it('uses month/year interval counts, preserves zero quantity and excludes metered charges', async () => {
    const yearly = fixed('year', {
      unit_amount: 12000,
      recurring: { usage_type: 'licensed', interval: 'year', interval_count: 2 },
    });
    const zero = { ...fixed('zero'), quantity: 0 };
    const metered = fixed('metered', { unit_amount: null, recurring: { usage_type: 'metered' } });
    assert.deepEqual((await mrr(recurring([yearly, zero, metered]))).values, [
      { currency: 'usd', amountMinor: 500, minorUnitExponent: 2 },
    ]);
  });

  it('sums yearly fractions before rounding the aggregate', async () => {
    const item = (id: string) =>
      fixed(id, { unit_amount: 5, recurring: { usage_type: 'licensed', interval: 'year', interval_count: 1 } });
    assert.equal((await mrr(recurring([item('a'), item('b'), item('c')]))).values?.[0].amountMinor, 1);
  });

  it('keeps currencies independent and applies verified zero-decimal and special API units', async () => {
    const stripe = fake(() =>
      page(
        ['usd', 'jpy', 'ugx', 'isk', 'huf', 'twd'].map((currency, index) =>
          invoice(`inv${index}`, { currency, amount_paid: 100 }),
        ),
      ),
    );
    const values = (await payments(stripe, NOW)).values!;
    assert.equal(values.length, 6);
    assert.equal(values.find((value) => value.currency === 'jpy')?.minorUnitExponent, 0);
    for (const currency of ['usd', 'ugx', 'isk', 'huf', 'twd'])
      assert.equal(values.find((value) => value.currency === currency)?.minorUnitExponent, 2);
  });

  it('withholds an authoritative total for unsupported units, tiers, fractional amounts or missing quantities', async () => {
    const invalid = [
      fixed('a', { currency: 'bhd' }),
      fixed('a', { billing_scheme: 'tiered' }),
      fixed('a', { unit_amount: 0.5 }),
      { ...fixed(), quantity: null },
      fixed('a', { recurring: { usage_type: 'licensed', interval: 'week', interval_count: 1 } }),
    ];
    for (const item of invalid)
      assert.deepEqual(
        [(await mrr(recurring([item]))).state, (await mrr(recurring([item]))).values],
        ['incomplete', null],
      );
  });

  it('never falls back to invoice creation when final payment time is missing', async () => {
    const result = await payments(
      fake(() => page([invoice('bad', { created: PAID, status_transitions: {} })])),
      NOW,
    );
    assert.deepEqual([result.state, result.values, result.days], ['incomplete', null, null]);
  });

  it('retains MRR when the invoice request fails independently', async () => {
    process.env.STRIPE_SECRET_KEY = 'sk_test';
    const result = await revenue(
      NOW,
      fake((path) => {
        if (path === '/invoices') throw new Error('Stripe: down');
        return page([subscription()]);
      }),
    );
    assert.deepEqual(
      [result.mrr.state, result.paidInvoices.state, result.paidInvoices.values],
      ['complete', 'unavailable', null],
    );
  });

  it('reports successful empty collections as measured zero', async () => {
    assert.deepEqual((await mrr(fake(() => page([])))).values, []);
    assert.deepEqual(
      (
        await payments(
          fake(() => page([])),
          NOW,
        )
      ).values,
      [],
    );
  });

  it('rejects repeated source rows even when pagination cursors advance', async () => {
    const stripe = fake((_, query) =>
      query.starting_after ? page([invoice('a'), invoice('b')]) : page([invoice('a')], true),
    );
    const result = await payments(stripe, NOW);
    assert.deepEqual([result.state, result.values], ['incomplete', null]);
  });

  it('stops at its request budget and never presents the partial subtotal', async () => {
    let count = 0;
    const stripe = fake(() => page([invoice(`inv${++count}`)], true));
    const result = await payments(stripe, NOW);
    assert.equal(stripe.calls.length, 20);
    assert.deepEqual([result.state, result.values], ['incomplete', null]);
  });
});
