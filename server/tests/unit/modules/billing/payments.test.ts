/**
 * Unit tests for saying when a person pays, or a payment fails.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { notePayment } from '../../../../src/modules/billing/payments.ts';
import { track } from '../../../../src/modules/telemetry/index.ts';

/** An invoice event. */
const event = (type: string, invoice: object, id = `evt_${Math.random()}`) => ({
  id,
  type,
  data: { object: invoice } as any,
});

/** A paid renewal of $99 for user u1. */
const RENEWAL = {
  amount_paid: 9900,
  amount_due: 9900,
  currency: 'usd',
  billing_reason: 'subscription_cycle',
  customer: 'cus_1',
  customer_email: 'ana@example.com',
  parent: { subscription_details: { metadata: { user_id: 'u1' } } },
};

describe('notePayment', () => {
  afterEach(() => mock.restoreAll());

  it('says a payment went through, a renewal as much as a new plan, naming who paid', () => {
    const paid = mock.method(track, 'paymentReceived', () => {});
    notePayment(event('invoice.paid', RENEWAL));
    assert.deepEqual(paid.mock.calls[0].arguments, [
      { id: 'u1', email: 'ana@example.com' },
      { amount_cents: 9900, currency: 'usd', reason: 'subscription_cycle' },
    ]);
  });

  it('says a payment failed, and which attempt it was', () => {
    const failed = mock.method(track, 'paymentFailed', () => {});
    notePayment(event('invoice.payment_failed', { ...RENEWAL, amount_paid: 0, attempt_count: 2 }));
    assert.deepEqual(failed.mock.calls[0].arguments[1], { amount_cents: 9900, currency: 'usd', attempt: 2 });
  });

  it('says nothing for an invoice for nothing, another event, or a delivery Stripe retried', () => {
    const paid = mock.method(track, 'paymentReceived', () => {});
    notePayment(event('invoice.paid', { ...RENEWAL, amount_paid: 0, amount_due: 0 }));
    notePayment(event('customer.subscription.updated', RENEWAL));
    notePayment(event('invoice.paid', RENEWAL, 'evt_same'));
    notePayment(event('invoice.paid', RENEWAL, 'evt_same'));
    assert.equal(paid.mock.calls.length, 1);
  });

  it('falls back to the Stripe customer when the invoice names no person', () => {
    const paid = mock.method(track, 'paymentReceived', () => {});
    notePayment(event('invoice.paid', { ...RENEWAL, parent: null, customer_email: null }));
    assert.deepEqual(paid.mock.calls[0].arguments[0], { id: 'cus_1', email: null });
  });
});
