/**
 * Says when a person pays, or a payment fails: a Slack line and a product
 * event for each invoice Stripe reports, the first month of a plan and every
 * renewal that goes through on its own. An invoice for nothing is not news.
 */
import { track } from '../telemetry/index.ts';
import type { StripeEvent } from './subscriptions.ts';
import { TOLD_MAX } from './constants.ts';

/** A Stripe invoice, as far as it is read here. */
type Invoice = Record<string, any>;

/** The person an invoice is for: the user id on its subscription, else the Stripe customer. */
function payer(invoice: Invoice) {
  const meta = invoice.parent?.subscription_details?.metadata ?? invoice.subscription_details?.metadata ?? {};
  return { id: String(meta.user_id || invoice.customer || 'unknown'), email: invoice.customer_email || null };
}

/** What each followed invoice event says. */
const SAID: Record<string, (invoice: Invoice) => void> = {
  'invoice.paid': (i) =>
    track.paymentReceived(payer(i), {
      amount_cents: i.amount_paid,
      currency: i.currency,
      reason: String(i.billing_reason || ''),
    }),
  'invoice.payment_failed': (i) =>
    track.paymentFailed(payer(i), {
      amount_cents: i.amount_due,
      currency: i.currency,
      attempt: Number(i.attempt_count) || 1,
    }),
};

/** Events already told, so a delivery Stripe retries is not said twice. ponytail: per replica, and forgets on restart. */
const told = new Set<string>();

/** Tells Slack and the analytics about a paid or failed invoice; any other event, or one for nothing, is ignored. */
export function notePayment(event: StripeEvent & { /** Stripe's event id. */ id?: string }) {
  const invoice = event.data?.object as Invoice | undefined;
  if (!invoice || !Object.hasOwn(SAID, event.type) || !(invoice.amount_paid || invoice.amount_due)) return;
  if (event.id && told.has(event.id)) return;
  if (told.size >= TOLD_MAX) told.clear();
  if (event.id) told.add(event.id);
  SAID[event.type](invoice);
}
