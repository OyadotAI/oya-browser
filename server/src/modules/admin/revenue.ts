/**
 * Revenue as Stripe has it, for the admin page: the payments of the days shown
 * and the monthly recurring revenue of the active subscriptions. Read live
 * from Stripe on the hosted deployment only; a self-hosted server has none.
 * A Stripe failure is shown on the page, never fails the overview.
 */
import { MS_PER_SECOND } from '../../platform/constants.ts';
import { hosted, stripeClient, stripeKey, type Stripe } from '../billing/index.ts';
import { DAY_CHARS, MONTHS_PER_YEAR, STRIPE_PAGE } from './constants.ts';

/** A Stripe object as it answers. */
type Obj = Record<string, any>;

/** Paid invoices since `sinceMs`, each as the UTC day it was paid and its cents. */
export async function payments(stripe: Stripe, sinceMs: number) {
  const query = { status: 'paid', created: { gte: Math.floor(sinceMs / MS_PER_SECOND) }, limit: STRIPE_PAGE };
  const { data = [] } = await stripe.get('/invoices', query);
  return data.map((i: Obj) => ({
    day: new Date((i.status_transitions?.paid_at || i.created) * MS_PER_SECOND).toISOString().slice(0, DAY_CHARS),
    cents: Number(i.amount_paid) || 0,
  }));
}

/** One subscription item's fixed monthly price in cents; metered items have none. */
function monthly(item: Obj) {
  const price = item.price || {};
  if (price.recurring?.usage_type !== 'licensed') return 0;
  const cents = (Number(price.unit_amount) || 0) * (Number(item.quantity) || 1);
  return price.recurring.interval === 'year' ? cents / MONTHS_PER_YEAR : cents;
}

/** Monthly recurring revenue in cents: the fixed prices of every active subscription, before usage. */
export async function mrr(stripe: Stripe) {
  const { data = [] } = await stripe.get('/subscriptions', { status: 'active', limit: STRIPE_PAGE });
  return Math.round(data.flatMap((s: Obj) => s.items?.data || []).reduce((n: number, i: Obj) => n + monthly(i), 0));
}

/** Revenue since `sinceMs`, or why it could not be read; off where Stripe is not set up. */
export async function revenue(sinceMs: number, stripe: Stripe = stripeClient(stripeKey)) {
  if (!hosted()) return { enabled: false, mrrCents: 0, payments: [], error: '' };
  try {
    const [list, mrrCents] = await Promise.all([payments(stripe, sinceMs), mrr(stripe)]);
    return { enabled: true, mrrCents, payments: list, error: '' };
  } catch (e) {
    return { enabled: true, mrrCents: 0, payments: [], error: e instanceof Error ? e.message : String(e) };
  }
}
