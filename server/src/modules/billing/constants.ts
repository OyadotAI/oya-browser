/**
 * The numbers billing runs on: what each plan includes, the Stripe meters its
 * usage is reported to, and what a model's tokens cost. Overage prices and the
 * LLM markup are not here: they are the plan's graduated prices in Stripe, so
 * changing a price never needs a deploy.
 */
import { BYTES_PER_MIB, MS_PER_MINUTE, SECONDS_PER_HOUR } from '../../platform/constants.ts';

/** Micro-USD in a cent: the hosted model is reported in cents of its cost. */
const MICRO_USD_PER_CENT = 10_000;
/** Seconds in a minute: cloud time is reported in minutes. */
const SECONDS_PER_MINUTE = 60;

/** One plan's allowances. */
export type Plan = {
  /** Cloud browser time included per period, in seconds. */
  cloudSeconds: number;
  /** Residential proxy traffic included per period, in bytes. */
  proxyBytes: number;
  /** Cloud browsers a person may run at once. */
  concurrent: number;
  /** Agent steps included per period. */
  steps: number;
  /** The hosted model's cost included per period, in micro-USD. */
  llmMicroUsd: number;
  /** Whether usage past the allowances is billed (true) or refused (false). */
  overage: boolean;
};

/** MiB in a GiB, which is what "GB" means on the pricing page. */
const MIB_PER_GIB = 1024;
/** Bytes in a GiB. */
const GIB = BYTES_PER_MIB * MIB_PER_GIB;
/** Developer's included cloud hours. */
const DEVELOPER_HOURS = 100;
/** Startup's included cloud hours. */
const STARTUP_HOURS = 500;
/** Startup's included proxy GiB. */
const STARTUP_PROXY_GIB = 5;

/** Every plan by name. Free is what anyone without a paid subscription is on. */
export const PLANS = {
  free: {
    cloudSeconds: SECONDS_PER_HOUR,
    proxyBytes: 0,
    concurrent: 3,
    steps: 500,
    llmMicroUsd: 500_000,
    overage: false,
  },
  developer: {
    ...{ cloudSeconds: DEVELOPER_HOURS * SECONDS_PER_HOUR, proxyBytes: GIB, concurrent: 25, steps: 5_000 },
    ...{ llmMicroUsd: 0, overage: true },
  },
  startup: {
    ...{ cloudSeconds: STARTUP_HOURS * SECONDS_PER_HOUR, proxyBytes: STARTUP_PROXY_GIB * GIB, concurrent: 100 },
    ...{ steps: 50_000, llmMicroUsd: 0, overage: true },
  },
} satisfies Record<string, Plan>;

/** A plan's name. */
export type PlanName = keyof typeof PLANS;

/** The plans a person can subscribe to. */
export const PAID_PLANS: PlanName[] = ['developer', 'startup'];

/** Subscription statuses that keep a paid plan's allowances. */
export const PAYING = new Set(['active', 'trialing']);
/** Statuses whose usage is still reported: past_due is paying late, not gone. */
export const REPORTABLE = new Set(['active', 'trialing', 'past_due']);

/** A reported usage quantity: the usage counter it comes from, the Stripe meter, and the unit it is sent in. */
export type Meter = {
  /** The usage counter summed for the period. */
  field: string;
  /** The Stripe meter's event name. */
  event: string;
  /** The counter's units per reported unit. */
  per: number;
};

/** What is reported to Stripe; the plan's prices there turn it into money. */
export const METERS: Meter[] = [
  { field: 'cloud_seconds', event: 'oya_cloud_minutes', per: SECONDS_PER_MINUTE },
  { field: 'residential_proxy_bytes', event: 'oya_proxy_mb', per: BYTES_PER_MIB },
  { field: 'hosted_llm_microusd', event: 'oya_model_cents', per: MICRO_USD_PER_CENT },
  { field: 'agent_steps', event: 'oya_agent_steps', per: 1 },
];

/** What a model's tokens cost, in micro-USD per token. */
export type TokenPrice = {
  /** Per token read. */
  input: number;
  /** Per token written. */
  output: number;
};

/** What the hosted models cost. */
export const MODEL_PRICES: Record<string, TokenPrice> = {
  'gpt-4o-mini': { input: 0.15, output: 0.6 },
  'gpt-4.1-mini': { input: 0.4, output: 1.6 },
  'gpt-4.1': { input: 2, output: 8 },
  'gpt-4o': { input: 2.5, output: 10 },
};

/** A model not in the table: the hosted key refuses to run one, so this only prices a call already made. */
export const UNKNOWN_MODEL_PRICE: TokenPrice = MODEL_PRICES['gpt-4o'];

/** Minutes between usage reports to Stripe. */
const REPORT_MINUTES = 5;
/** How often usage is reported to Stripe. */
export const REPORT_INTERVAL_MS = REPORT_MINUTES * MS_PER_MINUTE;
/** How old a webhook's signed timestamp may be, in seconds, before it reads as a replay. */
export const WEBHOOK_TOLERANCE_S = 300;
/** Stripe's API. */
export const STRIPE_API = 'https://api.stripe.com/v1';
/** The Stripe API version every call is pinned to; the webhook endpoint must be set to the same one. */
export const STRIPE_VERSION = '2025-03-31.basil';
/** Who to write to for more than the plans offer, or a self-hosted license. */
export const SALES_EMAIL = 'sales@getoya.ai';
/** Where in the console a person manages their plan. */
export const BILLING_PATH = '/dashboard/billing';
/** How Checkout looks: Oya's name, icon and colors, whatever else shares the Stripe account. */
export const CHECKOUT_BRANDING = {
  display_name: 'Oya Browser',
  icon: { type: 'url', url: 'https://oyabrowser.com/apple-icon.png' },
  background_color: '#0c0c0a',
  button_color: '#157a13',
  border_style: 'rounded',
};
/** Invoices the billing page lists. */
export const INVOICES_SHOWN = 24;
/** Characters of a hashed meter-event identifier: Stripe allows up to 100. */
export const IDENTIFIER_CHARS = 40;
/** Characters of an ISO time up to its hour: 2026-03-10T14. */
export const HOUR_CHARS = 13;
/** Minutes an admitted cloud browser holds its place before it connects: a sandbox dials in within about 90 s. */
const RESERVATION_MINUTES = 3;
/** How long an admitted cloud browser holds its place. */
export const RESERVATION_MS = RESERVATION_MINUTES * MS_PER_MINUTE;
/** Payment events remembered, so a retried delivery is not announced twice. */
export const TOLD_MAX = 1_000;
