/**
 * Billing's settings, read from the environment in this one place. Billing is
 * on only where STRIPE_SECRET_KEY is set: that is the hosted deployment. A
 * self-hosted server has none, so it has no plans and runs under the
 * self-hosted license instead.
 */
import { PAID_PLANS, type PlanName } from './constants.ts';

/** Whether this is the hosted deployment, which bills through Stripe. */
export const hosted = () => Boolean(process.env.STRIPE_SECRET_KEY);

/** The Stripe secret key. */
export const stripeKey = () => process.env.STRIPE_SECRET_KEY || '';

/** The secret Stripe signs webhooks with. */
export const webhookSecret = () => process.env.STRIPE_WEBHOOK_SECRET || '';

/** The Customer portal configuration Oya's customers get; the account's default without one. */
export const portalConfiguration = () => process.env.STRIPE_PORTAL_CONFIGURATION || undefined;

/** A comma list from the environment, trimmed, without empties. */
const list = (name: string) =>
  String(process.env[name] || '')
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);

/** A plan's Stripe price ids, base first: STRIPE_PRICES_<PLAN> = base,minutes,mb,llm,steps. */
export const pricesFor = (plan: PlanName) => list(`STRIPE_PRICES_${plan.toUpperCase()}`);

/** The plan a subscription's base price belongs to; free for a price no plan names. */
export const planOfPrice = (priceId: string): PlanName => PAID_PLANS.find((p) => pricesFor(p)[0] === priceId) || 'free';
