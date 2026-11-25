/**
 * Takes back self-hosted licenses whose money went back: a fully refunded or
 * disputed payment revokes every key its subscription issued, so paying for a
 * year and then disputing the charge does not keep the browsers. A revoked key
 * stops at its server's next daily ping, and the subscription is set to end so
 * it charges no more.
 */
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { LICENSE_KIND } from './constants.ts';
import type { Stripe } from './stripe.ts';
import type { StripeEvent } from './subscriptions.ts';

/** What revoking needs. */
export type RevocationDeps = {
  /** Stripe, to find the invoice a payment paid. */
  stripe: Stripe;
  /** The license issued for an invoice, or null. */
  byInvoice(invoiceId: string): Promise<Record<string, any> | null>;
  /** Revokes the live licenses bought under a subscription. */
  revokeUnder(subscriptionId: string, at: string): Promise<number>;
  /** The time, in ms. */
  now(): number;
};

/** A Stripe object, as far as it is read here. */
type StripeObject = Record<string, any>;

/** The payment an event took back, or null: a full refund, or any dispute. Partial refunds keep the key. */
const TAKEN_BACK: Record<string, (o: StripeObject) => StripeObject | null> = {
  'charge.refunded': (charge) => (charge.refunded ? charge : null),
  'charge.dispute.created': (dispute) => dispute,
};

/** The PaymentIntent id a charge or dispute names. */
const intentOf = (o: StripeObject) => (typeof o.payment_intent === 'string' ? o.payment_intent : o.payment_intent?.id);

/**
 * The invoice a payment paid. API 2025-03-31 took `invoice` off the charge, so
 * it is asked of Stripe's invoice payments; an older charge still carries it.
 */
async function invoiceOf(stripe: Stripe, o: StripeObject): Promise<string | null> {
  if (typeof o.invoice === 'string') return o.invoice;
  const intent = intentOf(o);
  if (!intent) return null;
  const query = { payment: { type: 'payment_intent', payment_intent: intent }, limit: 1 };
  const found = await stripe.get('/invoice_payments', query);
  const invoice = found?.data?.[0]?.invoice;
  return typeof invoice === 'string' ? invoice : (invoice?.id ?? null);
}

/** Whether a Stripe invoice is for a self-hosted license: its subscription's metadata says so. */
const isLicense = (invoice: StripeObject) =>
  (invoice.parent?.subscription_details?.metadata ?? invoice.subscription_details?.metadata)?.kind === LICENSE_KIND;

/**
 * A refund for a license invoice whose key is not recorded yet (its invoice.paid
 * is still being retried) answers 503, so Stripe retries the refund once the key
 * exists instead of the key outliving its money.
 */
async function awaitKey(stripe: Stripe, invoiceId: string) {
  if (!isLicense(await stripe.get(`/invoices/${invoiceId}`))) return;
  throw new HttpError(Status.UNAVAILABLE, `License for ${invoiceId} not issued yet; retry`, { code: 'retry' });
}

/** Ends the subscription at its period's close so it charges no more; a failure is said, the revocation stands. */
const stopBilling = (stripe: Stripe, subscription: string) =>
  stripe
    .post(`/subscriptions/${subscription}`, { cancel_at_period_end: true })
    .catch((e) => console.warn(`[billing] could not end ${subscription} after revoking: ${e.message}`));

/** Revokes the licenses a refunded or disputed payment bought; any other event, or a payment for anything else, is left. */
export async function revokeForEvent(event: StripeEvent, deps: RevocationDeps) {
  const object = event.data?.object;
  if (!object || !Object.hasOwn(TAKEN_BACK, event.type)) return;
  const payment = TAKEN_BACK[event.type](object);
  const invoice = payment && (await invoiceOf(deps.stripe, payment));
  if (!invoice) return;
  const row = await deps.byInvoice(invoice);
  if (!row) return awaitKey(deps.stripe, invoice);
  await revokeSubscription(deps, row.stripe_subscription_id, `${event.type} on invoice ${invoice}`);
}

/** Revokes a subscription's keys and stops its billing, saying why. */
export async function revokeSubscription(deps: RevocationDeps, subscription: string, why: string) {
  const revoked = await deps.revokeUnder(subscription, new Date(deps.now()).toISOString());
  await stopBilling(deps.stripe, subscription);
  console.warn(`[billing] ${why}: revoked ${revoked} license(s) of ${subscription}, set to end`);
}
