/**
 * Unit tests for a person's invoices from Stripe: those issued, and the next
 * one line by line.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { Invoices } from '../../../../src/modules/billing/invoices.ts';

/** Invoices over a subscription row and a Stripe that answers from the test. */
function fixture(row: any, answers: Record<string, any> = {}) {
  const calls: any[] = [];
  const stripe = {
    get: async (path, query) => (calls.push({ path, query }), answers[path]),
    post: async (path, params) => (calls.push({ path, params }), answers[path]),
  };
  return { inv: new Invoices({ find: async () => row, stripe }), calls };
}

/** Seconds for 10 March 2026. */
const T = Date.UTC(2026, 2, 10) / 1000;

describe('Invoices', () => {
  it('lists issued invoices with their amounts and links, newest first as Stripe gives them', async () => {
    const raw = {
      id: 'in_1',
      number: 'OYA-1',
      status: 'paid',
      created: T,
      currency: 'usd',
      total: 2000,
      amount_paid: 2000,
      hosted_invoice_url: 'h',
      invoice_pdf: 'p',
      period_start: T,
      period_end: T,
    };
    const { inv, calls } = fixture({ stripe_customer_id: 'cus_1' }, { '/invoices': { data: [raw] } });
    const { invoices } = await inv.list('u1');
    assert.deepEqual(invoices[0], {
      ...{ id: 'in_1', number: 'OYA-1', status: 'paid', created: '2026-03-10T00:00:00.000Z', currency: 'usd' },
      ...{
        total: 2000,
        amount_paid: 2000,
        url: 'h',
        pdf: 'p',
        period_start: '2026-03-10T00:00:00.000Z',
        period_end: '2026-03-10T00:00:00.000Z',
      },
    });
    assert.equal(calls[0].query.customer, 'cus_1');
  });

  it('has no invoices for someone who never subscribed, without asking Stripe', async () => {
    const { inv, calls } = fixture(null);
    assert.deepEqual(await inv.list('u1'), { invoices: [] });
    assert.deepEqual(await inv.upcoming('u1'), { upcoming: null });
    assert.equal(calls.length, 0);
  });

  it('shows the next invoice line by line', async () => {
    const preview = {
      total: 2350,
      currency: 'usd',
      period_end: T,
      lines: {
        data: [
          { description: 'Agent steps', quantity: 6000, amount: 200 },
          { description: 'Developer', amount: 2000 },
        ],
      },
    };
    const { inv, calls } = fixture(
      { stripe_customer_id: 'cus_1', stripe_subscription_id: 'sub_1', status: 'active' },
      { '/invoices/create_preview': preview },
    );
    const { upcoming } = await inv.upcoming('u1');
    assert.equal(upcoming.total, 2350);
    assert.deepEqual(upcoming.lines, [
      { description: 'Agent steps', quantity: 6000, amount: 200 },
      { description: 'Developer', quantity: null, amount: 2000 },
    ]);
    assert.deepEqual(calls[0].params, { customer: 'cus_1', subscription: 'sub_1' });
  });

  it('shows no next invoice once the subscription is canceled', async () => {
    const { inv } = fixture({ stripe_customer_id: 'cus_1', stripe_subscription_id: 'sub_1', status: 'canceled' });
    assert.deepEqual(await inv.upcoming('u1'), { upcoming: null });
  });
});
