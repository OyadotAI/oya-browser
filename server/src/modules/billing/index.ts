/**
 * Billing's facade: plans for the hosted cloud, paid through Stripe. The
 * composition root builds it once with `createBilling`; everything else
 * reaches it through the container.
 */
import { stripeClient } from './stripe.ts';
import { hosted, stripeKey } from './config.ts';
import { Entitlements, type EntitlementDeps } from './entitlements.ts';
import { Subscriptions } from './subscriptions.ts';
import { UsageReporter } from './reporter.ts';
import { Invoices } from './invoices.ts';
import { REPORT_INTERVAL_MS, BILLING_PATH } from './constants.ts';
import * as repository from './repository.ts';
import * as licenseRepository from './license-repository.ts';
import { LicenseSales } from './licenses.ts';
import { mint } from '../../platform/license/index.ts';
import { sealText, openText } from '../../platform/secrets.ts';

export { hosted, stripeKey } from './config.ts';
export { llmCost } from './cost.ts';
export { verifySignature, stripeClient, type Stripe } from './stripe.ts';
export { Entitlements, type EntitlementDeps } from './entitlements.ts';
export { Subscriptions } from './subscriptions.ts';
export { UsageReporter } from './reporter.ts';
export { Invoices } from './invoices.ts';
export { LicenseSales } from './licenses.ts';
export { PLANS } from './constants.ts';
export { billingRoutes, billingWebhook } from './routes.ts';

/** What the server hands billing: who owns what, what runs, and the console's address. */
export type BillingWiring = Omit<EntitlementDeps, 'find' | 'usageSince' | 'upgradeUrl' | 'now'> & {
  /** The console's public address. */
  consoleUrl(): string;
};

/** Builds billing's parts over the subscriptions and licenses tables and Stripe. */
export function createBilling(wiring: BillingWiring, fetchFn: typeof fetch = fetch) {
  const back = () => `${wiring.consoleUrl()}${BILLING_PATH}`;
  const shared = { ...repository, now: () => Date.now(), stripe: stripeClient(stripeKey, fetchFn) };
  const subscriptions = new Subscriptions({ ...shared, returnUrl: back });
  const entitlements = new Entitlements({ ...wiring, ...shared, upgradeUrl: back });
  const licenses = licenseSales(shared, subscriptions, back);
  return { entitlements, subscriptions, reporter: new UsageReporter(shared), invoices: new Invoices(shared), licenses };
}

/** Self-hosted license sales, opening Checkout the way plans do. */
const licenseSales = (shared, subscriptions: Subscriptions, back: () => string) =>
  new LicenseSales({
    ...{ ...licenseRepository, stripe: shared.stripe, now: shared.now, returnUrl: back, mint },
    ...{ openCheckout: (params) => subscriptions.openCheckout(params), seal: sealText, open: openText },
  });

/** Billing as the container provides it. */
export type Billing = ReturnType<typeof createBilling>;

/** Reports usage to Stripe on an interval, on the hosted deployment only; answers the stop function. */
export function startReporting(reporter: UsageReporter) {
  if (!hosted()) return () => {};
  const timer = setInterval(() => reporter.report().catch(() => {}), REPORT_INTERVAL_MS);
  timer.unref?.();
  return () => clearInterval(timer);
}

export { overrideFor, saveOverride, saveGrant, grantById, grantsFor, creditsFor } from './adjustment-repository.ts';
export { isPlan, includedFor } from './adjustments.ts';
export { standingOf } from './standing.ts';
export { find as findSubscription } from './repository.ts';
