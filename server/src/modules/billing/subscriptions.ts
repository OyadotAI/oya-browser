/**
 * Subscribing and managing a plan, through Stripe's own pages: Checkout to
 * subscribe, the Customer Portal to change card, plan or cancel. Stripe tells
 * the server what changed by webhook, and that is the only way a plan changes
 * here. Admin access overrides are stored separately from the paid subscription.
 */
import { HttpError, invalid, notFound } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { MS_PER_SECOND } from '../../platform/constants.ts';
import { portalConfiguration, pricesFor, planOfPrice } from './config.ts';
import { CHECKOUT_BRANDING, PAID_PLANS, type PlanName } from './constants.ts';
import { includedFor, type CreditReader } from './adjustments.ts';
import { standingOf, type Standing } from './standing.ts';
import type { Stripe } from './stripe.ts';
import type { Subscription } from './repository.ts';

/** What subscribing needs. */
export type SubscriptionDeps = CreditReader & {
  /** A person's subscription row. */
  find(userId: string): Promise<Subscription | null>;
  /** Writes what Stripe says of a person's subscription. */
  saveStripe(userId: string, columns: Record<string, unknown>): Promise<void>;
  /** A person's billed usage since an ISO time. */
  usageSince(userId: string, since: string): Promise<Record<string, number>>;
  /** Stripe. */
  stripe: Stripe;
  /** Where Checkout and the Portal send the person back to. */
  returnUrl(): string;
  /** The time, in ms. */
  now(): number;
};

/** A Stripe subscription object, as far as it is read here. */
type StripeSubscription = Record<string, any>;

/** A Stripe timestamp (seconds) as an ISO time, or null. */
const iso = (seconds: unknown) => (Number(seconds) ? new Date(Number(seconds) * MS_PER_SECOND).toISOString() : null);

/** The plan a subscription is for: the one whose base price is among its items. */
function planOf(sub: StripeSubscription): PlanName {
  const prices: string[] = (sub.items?.data || []).map((i) => i.price?.id);
  return prices.map(planOfPrice).find((p) => p !== 'free') || 'free';
}

/** The subscription's current period: on its items from API 2025-03-31, on itself before. */
function periodOf(sub: StripeSubscription) {
  const item = (sub.items?.data || []).find((i) => i.current_period_start) || {};
  const start = item.current_period_start ?? sub.current_period_start;
  return { period_start: iso(start), period_end: iso(item.current_period_end ?? sub.current_period_end) };
}

/** The row's columns as Stripe has them; an ended subscription is Free and canceled. */
const fromStripe = (sub: StripeSubscription, ended: boolean) => ({
  ...{ stripe_customer_id: sub.customer, stripe_subscription_id: sub.id },
  ...{ plan: ended ? 'free' : planOf(sub), status: ended ? 'canceled' : sub.status, ...periodOf(sub) },
});

/** Checkout's line items: the base price once, the metered prices with no quantity (usage decides it). */
const lineItems = (prices: string[]) => prices.map((price, i) => (i === 0 ? { price, quantity: 1 } : { price }));

/** A paid plan's Stripe prices, or the 400 or 503 that says it cannot be bought. */
function pricesOnSale(plan: string) {
  if (!PAID_PLANS.includes(plan as PlanName)) throw invalid('plan', PAID_PLANS.join(' or '), plan);
  const prices = pricesFor(plan as PlanName);
  if (!prices.length) throw new HttpError(Status.UNAVAILABLE, `The ${plan} plan is not on sale yet.`);
  return prices;
}

/** A Stripe event, as far as it is read here. */
export type StripeEvent = {
  /** What happened, such as customer.subscription.updated. */
  type: string;
  /** When Stripe made the event, in seconds: events can arrive out of order. */
  created?: number;
  /** What it happened to. */
  data?: {
    /** The subscription. */
    object?: StripeSubscription;
  };
};

/** A person's plan in Stripe, and the webhook that keeps it in step. */
export class Subscriptions {
  /** What subscribing reads and writes. */
  declare deps: SubscriptionDeps;

  /** Everything goes through `deps`. */
  constructor(deps: SubscriptionDeps) {
    this.deps = deps;
  }

  /** A Checkout page for `plan`, which the person is sent to. */
  async checkout(userId: string, plan: string) {
    const prices = pricesOnSale(plan);
    const customer = (await this.deps.find(userId))?.stripe_customer_id || undefined;
    const back = this.deps.returnUrl();
    const session = await this.openCheckout({
      ...{ mode: 'subscription', line_items: lineItems(prices), client_reference_id: userId, customer },
      ...{ subscription_data: { metadata: { user_id: userId } }, success_url: back, cancel_url: back },
    });
    return { url: session.url };
  }

  /** Opens Checkout in Oya's look; should Stripe refuse the look, the page still opens, plain. */
  openCheckout(params: Record<string, unknown>) {
    const branded = { ...params, branding_settings: CHECKOUT_BRANDING };
    return this.deps.stripe
      .post('/checkout/sessions', branded)
      .catch((e) =>
        /branding/i.test(e.message) ? this.deps.stripe.post('/checkout/sessions', params) : Promise.reject(e),
      );
  }

  /** A Customer Portal page, where the person changes card, sees invoices or cancels: Oya's own portal where one is set. */
  async portal(userId: string) {
    const customer = (await this.deps.find(userId))?.stripe_customer_id;
    if (!customer) throw notFound('Subscription');
    const params = { customer, configuration: portalConfiguration(), return_url: this.deps.returnUrl() };
    const session = await this.deps.stripe.post('/billing_portal/sessions', params);
    return { url: session.url };
  }

  /** The person's plan, its allowances and what they used this period, for the console. */
  async summary(userId: string) {
    const row = await this.deps.find(userId);
    const standing = standingOf(userId, row, this.deps.now());
    const { since } = standing;
    const included = includedFor(standing, await this.deps.creditsFor?.(userId, since));
    const used = await this.deps.usageSince(userId, since);
    return { ...summaryFields(standing, row), included, used };
  }

  /** The person's plan name alone: one row read, for places that only name the plan. */
  async plan(userId: string) {
    return standingOf(userId, await this.deps.find(userId), this.deps.now()).plan;
  }

  /** Applies a verified webhook event; events billing does not follow are ignored. */
  async applyEvent(event: StripeEvent) {
    const sub = event.data?.object;
    if (!sub || !Object.hasOwn(FOLLOWED, event.type)) return;
    await this.sync(sub, FOLLOWED[event.type], Number(event.created) || 0);
  }

  /** Writes what Stripe says of a subscription to its person's row, unless the row already knows better. */
  async sync(sub: StripeSubscription, ended: boolean, eventAt: number) {
    const userId = sub.metadata?.user_id;
    if (!userId) return;
    if (superseded(await this.deps.find(userId), sub, ended, eventAt)) return;
    await this.deps.saveStripe(userId, { ...fromStripe(sub, ended), stripe_event_at: eventAt });
  }
}

/**
 * Whether an event says less than the row already knows: an older event about
 * the same subscription, or the end of a subscription the person has since
 * replaced with another.
 */
function superseded(row: Subscription | null, sub: StripeSubscription, ended: boolean, eventAt: number) {
  if (!row?.stripe_subscription_id) return false;
  if (row.stripe_subscription_id !== sub.id) return ended;
  return Number(row.stripe_event_at) > eventAt;
}

/** The subscription events billing follows, and whether each ends the subscription. */
const FOLLOWED: Record<string, boolean> = {
  'customer.subscription.created': false,
  'customer.subscription.updated': false,
  'customer.subscription.deleted': true,
};

/** Common summary fields keep payment management separate from complimentary access. */
function summaryFields(standing: Standing, row: Subscription | null) {
  const { plan, status, since } = standing;
  return {
    ...{ enabled: true, plan, status, since },
    until: row?.period_end ?? null,
    canManage: Boolean(row?.stripe_customer_id),
  };
}
