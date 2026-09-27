/**
 * A paying person's invoices, from Stripe: the ones already issued, and the
 * next one as it stands, line by line. Someone who never subscribed has none.
 */
import { MS_PER_SECOND } from '../../platform/constants.ts';
import { INVOICES_SHOWN } from './constants.ts';
import type { Stripe } from './stripe.ts';
import type { Subscription } from './repository.ts';

/** What reading invoices needs. */
export type InvoiceDeps = {
  /** A person's subscription row. */
  find(userId: string): Promise<Subscription | null>;
  /** Stripe. */
  stripe: Stripe;
};

/** A Stripe object, as far as it is read here. */
type Raw = Record<string, any>;

/** A Stripe timestamp (seconds) as an ISO time, or null. */
const iso = (seconds: unknown) => (Number(seconds) ? new Date(Number(seconds) * MS_PER_SECOND).toISOString() : null);

/** An issued invoice as the billing page lists it. */
const issued = (i: Raw) => ({
  ...{ id: i.id, number: i.number, status: i.status, created: iso(i.created), currency: i.currency },
  ...{ total: i.total, amount_paid: i.amount_paid, url: i.hosted_invoice_url, pdf: i.invoice_pdf },
  ...{ period_start: iso(i.period_start), period_end: iso(i.period_end) },
});

/** One line of an invoice: what it is for, how many, and what it comes to, in cents. */
const lineOf = (l: Raw) => ({ description: l.description, quantity: l.quantity ?? null, amount: l.amount });

/** A person's invoices. */
export class Invoices {
  /** What reading invoices reads. */
  declare deps: InvoiceDeps;

  /** Everything goes through `deps`. */
  constructor(deps: InvoiceDeps) {
    this.deps = deps;
  }

  /** Invoices already issued, newest first; none for someone who never subscribed. */
  async list(userId: string) {
    const customer = (await this.deps.find(userId))?.stripe_customer_id;
    if (!customer) return { invoices: [] };
    const page = await this.deps.stripe.get('/invoices', { customer, limit: INVOICES_SHOWN });
    return { invoices: (page.data || []).map(issued) };
  }

  /** The next invoice as it stands now, line by line; null without a subscription. */
  async upcoming(userId: string) {
    const row = await this.deps.find(userId);
    if (!row?.stripe_customer_id || !row.stripe_subscription_id || row.status === 'canceled') return { upcoming: null };
    const params = { customer: row.stripe_customer_id, subscription: row.stripe_subscription_id };
    const next = await this.deps.stripe.post('/invoices/create_preview', params);
    const lines = (next.lines?.data || []).map(lineOf);
    return { upcoming: { total: next.total, currency: next.currency, date: iso(next.period_end), lines } };
  }
}
