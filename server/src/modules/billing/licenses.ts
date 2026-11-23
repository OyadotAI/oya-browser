/**
 * Self-hosted licenses sold through Stripe, so a self-hoster past the free cap
 * can buy more browsers themselves instead of writing to sales. Checkout opens
 * a subscription billed per browser; each paid invoice issues a key good for
 * its period plus a grace, which the buyer sees on the console's billing page.
 */
import { HttpError, invalid, notFound } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { MS_PER_SECOND } from '../../platform/constants.ts';
import { SELF_SERVE_MAX, type Minted } from '../../platform/license/index.ts';
import {
  LICENSE_INTERVALS,
  licenseIntervalOf,
  licensePrice,
  portalConfiguration,
  type LicenseInterval,
} from './config.ts';
import { LICENSE_GRACE_S, LICENSE_KIND, LICENSE_LIVE, LICENSE_MIN } from './constants.ts';
import type { LicenseRow } from './license-repository.ts';
import { revokeForEvent, revokeSubscription } from './license-revocation.ts';
import type { StripeEvent } from './subscriptions.ts';
import type { Stripe } from './stripe.ts';

/** What selling licenses needs. */
export type LicenseDeps = {
  /** Stripe. */
  stripe: Stripe;
  /** Opens a Checkout page in Oya's look (Subscriptions.openCheckout). */
  openCheckout(params: Record<string, unknown>): Promise<any>;
  /** Signs a license (platform/license mint). */
  mint(request: Record<string, unknown>): Minted;
  /** Seals a key at rest under a scope. */
  seal(scope: string, value: string): string;
  /** Opens a sealed key. */
  open(scope: string, sealed: string): string;
  /** The license issued for an invoice, or null. */
  byInvoice(invoiceId: string): Promise<LicenseRow | null>;
  /** A person's live licenses, newest first. */
  forUser(userId: string): Promise<LicenseRow[]>;
  /** Stores a license. */
  insert(row: LicenseRow): Promise<void>;
  /** Whether a license under a subscription was revoked. */
  revokedUnder(subscriptionId: string): Promise<boolean>;
  /** Revokes the live licenses under a subscription; answers how many. */
  revokeUnder(subscriptionId: string, at: string): Promise<number>;
  /** The billing page, where Checkout and the Portal send the person back. */
  returnUrl(): string;
  /** The time, in ms. */
  now(): number;
};

/** A Stripe object, as far as it is read here. */
type StripeObject = Record<string, any>;

/** The seal scope for a person's keys: a key sealed for one person never opens for another. */
const scopeOf = (userId: string) => `license:${userId}`;

/** A Stripe timestamp (seconds) as an ISO time. */
const iso = (seconds: number) => new Date(seconds * MS_PER_SECOND).toISOString();

/** The browser count asked for, or the 400 that says what it must be. */
function browsersAsked(value: unknown) {
  const n = Number(value);
  if (Number.isInteger(n) && n >= LICENSE_MIN && n <= SELF_SERVE_MAX) return n;
  throw invalid('browsers', `a whole number from ${LICENSE_MIN} to ${SELF_SERVE_MAX}`, value);
}

/** The per-browser price for a billing period, or the 400 or 503 that says it cannot be bought. */
function priceOnSale(interval: unknown) {
  if (!LICENSE_INTERVALS.includes(interval as LicenseInterval))
    throw invalid('interval', LICENSE_INTERVALS.join(' or '), interval);
  const price = licensePrice(interval as LicenseInterval);
  if (!price) throw new HttpError(Status.UNAVAILABLE, 'Self-hosted licenses are not on sale yet.');
  return price;
}

/** The subscription details an invoice carries: on `parent` from API 2025-03-31, on itself before. */
const detailsOf = (invoice: StripeObject) => invoice.parent?.subscription_details ?? invoice.subscription_details ?? {};

/** A line's price id: under `pricing` from API 2025-03-31, on `price` before. */
const priceOfLine = (line: StripeObject) => line.pricing?.price_details?.price ?? line.price?.id;

/**
 * The invoice line that is the license for the period paid, or undefined for an invoice of anything else.
 * After a quantity change Stripe adds proration lines for the old period first; the latest period is the one paid.
 */
const licenseLine = (invoice: StripeObject) =>
  (invoice.lines?.data || [])
    .filter((line) => licenseIntervalOf(priceOfLine(line)))
    .sort((a, b) => Number(b.period?.end) - Number(a.period?.end))[0];

/** A subscription's current period end, in seconds: on its items from API 2025-03-31, on itself before. */
const periodEndOf = (sub: StripeObject) => sub.items?.data?.[0]?.current_period_end ?? sub.current_period_end;

/** Whether a Stripe subscription is still being paid for and not set to end. */
const stillLive = (sub: StripeObject | null) =>
  Boolean(sub && LICENSE_LIVE.includes(sub.status) && !sub.cancel_at_period_end);

/** Selling self-hosted licenses, issuing their keys as invoices are paid, and showing them to their buyer. */
export class LicenseSales {
  /** What selling reads and writes. */
  declare deps: LicenseDeps;

  /** Everything goes through `deps`. */
  constructor(deps: LicenseDeps) {
    this.deps = deps;
  }

  /** A Checkout page for `browsers` browsers billed each `interval`; refused while another license is being paid for. */
  async checkout(userId: string, browsers: unknown, interval: unknown) {
    const quantity = browsersAsked(browsers);
    const price = priceOnSale(interval);
    const [latest] = await this.deps.forUser(userId);
    if (stillLive(await this.subscriptionOf(latest))) throw alreadyLicensed();
    const back = this.deps.returnUrl();
    const session = await this.deps.openCheckout(checkoutParams({ userId, price, quantity, back }, latest));
    return { url: session.url };
  }

  /** Issues a key for a paid license invoice, or revokes the keys a refund or dispute took back; anything else is ignored. */
  async applyEvent(event: StripeEvent) {
    const invoice = event.data?.object;
    if (event.type !== 'invoice.paid') return revokeForEvent(event, this.deps);
    if (!invoice || detailsOf(invoice).metadata?.kind !== LICENSE_KIND) return;
    const line = licenseLine(invoice);
    if (!line) return warn(`license invoice ${invoice.id} has no current license price; no key issued`);
    if (await this.deps.byInvoice(invoice.id)) return;
    await this.issueUnlessRevoked(invoice, line);
  }

  /** Issues the key, unless the subscription had a key revoked (refund, dispute or admin): then no key, and its billing is stopped. */
  async issueUnlessRevoked(invoice: StripeObject, line: StripeObject) {
    const { subscription } = detailsOf(invoice);
    if (!(await this.deps.revokedUnder(subscription))) return this.deps.insert(this.issue(invoice, line));
    await revokeSubscription(
      this.deps,
      subscription,
      `invoice ${invoice.id} paid on a revoked subscription, refund it by hand`,
    );
  }

  /** Mints the key for one paid invoice line and answers its row, the key sealed to its buyer. */
  issue(invoice: StripeObject, line: StripeObject): LicenseRow {
    const bought = boughtColumns(invoice, line);
    const expiresAt = iso(Number(line.period?.end) + LICENSE_GRACE_S);
    const licensee = invoice.customer_email || bought.user_id;
    const { license, token } = this.deps.mint({ licensee, maxConcurrent: line.quantity, expiresAt });
    const row = { id: license.id, licensee, max_concurrent: license.maxConcurrent, expires_at: license.expiresAt };
    const made = { created_at: new Date(this.deps.now()).toISOString(), created_by: 'stripe', revoked_at: null };
    return { ...row, ...made, ...bought, key_sealed: this.deps.seal(scopeOf(bought.user_id), token) };
  }

  /** The person's current license with its key, the previous key's end while it still works, and its billing state. */
  async view(userId: string) {
    const [current, previous] = await this.deps.forUser(userId);
    if (!current) return { enabled: true, license: null };
    const billing = billingOf(await this.subscriptionOf(current));
    const stillWorks = previous && new Date(previous.expires_at).getTime() > this.deps.now();
    const previousExpiresAt = stillWorks ? previous.expires_at : null;
    return { enabled: true, license: { ...shown(current, this.deps.open), ...billing, previousExpiresAt } };
  }

  /** The Stripe subscription a license row was bought under, or null when Stripe cannot say. */
  subscriptionOf(row?: LicenseRow): Promise<StripeObject | null> {
    if (!row?.stripe_subscription_id) return Promise.resolve(null);
    return this.deps.stripe.get(`/subscriptions/${row.stripe_subscription_id}`).catch(() => null);
  }

  /** A Customer Portal page for the license's card, receipts and cancellation. */
  async portal(userId: string) {
    const [latest] = await this.deps.forUser(userId);
    if (!latest?.stripe_customer_id) throw notFound('License');
    const params = { customer: latest.stripe_customer_id, configuration: portalConfiguration() };
    const session = await this.deps.stripe.post('/billing_portal/sessions', {
      ...params,
      return_url: `${this.deps.returnUrl()}#self-hosted`,
    });
    return { url: session.url };
  }
}

/** What a Checkout page is opened for. */
type Purchase = {
  /** Who buys. */
  userId: string;
  /** The per-browser price. */
  price: string;
  /** How many browsers. */
  quantity: number;
  /** The billing page. */
  back: string;
};

/** Checkout's parameters: one line, per browser, tagged as a license so the plan webhook leaves it alone. */
const checkoutParams = ({ userId, price, quantity, back }: Purchase, latest?: LicenseRow) => ({
  ...{ mode: 'subscription', line_items: [{ price, quantity }], client_reference_id: userId },
  ...{ customer: latest?.stripe_customer_id || undefined, success_url: `${back}?license=paid#self-hosted` },
  ...{ cancel_url: `${back}#self-hosted`, subscription_data: { metadata: { user_id: userId, kind: LICENSE_KIND } } },
});

/** The Stripe columns of a bought license: who paid, under which subscription, for which invoice. */
function boughtColumns(invoice: StripeObject, line: StripeObject) {
  const { metadata, subscription } = detailsOf(invoice);
  return {
    ...{ user_id: metadata.user_id, stripe_customer_id: invoice.customer, stripe_subscription_id: subscription },
    ...{ stripe_invoice_id: invoice.id, interval: licenseIntervalOf(priceOfLine(line)) },
  };
}

/** Says why a paid license invoice issued no key: someone has to look. */
const warn = (why: string) => console.warn(`[billing] ${why}`);

/** The 409 for a second license while one is being paid for: more browsers go through the portal or sales. */
const alreadyLicensed = () =>
  new HttpError(Status.CONFLICT, 'You already have a self-hosted license. Cancel it before buying another.', {
    code: 'license_exists',
  });

/** A license row as its buyer sees it, the key opened. */
const shown = (row: LicenseRow, open: LicenseDeps['open']) => ({
  ...{ id: row.id, browsers: Number(row.max_concurrent), interval: row.interval, expiresAt: row.expires_at },
  ...{ issuedAt: row.created_at, key: open(scopeOf(row.user_id), row.key_sealed) },
});

/** What Stripe says of the subscription: its state, and when it renews unless it is set to end. */
const billingOf = (sub: StripeObject | null) => ({
  status: sub?.status ?? null,
  cancelAtPeriodEnd: Boolean(sub?.cancel_at_period_end),
  renewsAt: sub && periodEndOf(sub) && !sub.cancel_at_period_end ? iso(Number(periodEndOf(sub))) : null,
});
