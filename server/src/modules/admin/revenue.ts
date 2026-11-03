/** Independent, currency-aware financial reads never block core administrative activity. */
import { hosted, stripeClient, stripeKey, type Stripe } from '../billing/index.ts';
import { MS_PER_SECOND } from '../../platform/constants.ts';
import { adminPeriods, utcDay } from './periods.ts';
import { lastDays } from './growth.ts';
import { DAYS_SHOWN } from './constants.ts';
import { addMoney, checkedInteger, fixedMonthly, moneyValues } from './financial-money.ts';
import {
  checkedPage,
  financialBudget,
  financialList,
  IncompleteFinancial,
  type FinancialBudget,
} from './financial-pages.ts';
import type { FinancialMetric, PaidInvoices } from './financial-types.ts';

/** Non-complete money is unavailable, never a measured zero or a misleading subtotal. */
const missing = (state: FinancialMetric['state'], reason: string): FinancialMetric => ({ state, values: null, reason });

/** Bounded and unsupported reads are incomplete; upstream failures remain unavailable. */
function failedMetric(error: unknown, budget: FinancialBudget) {
  const incomplete = error instanceof IncompleteFinancial || budget.signal.aborted;
  return missing(
    incomplete ? 'incomplete' : 'unavailable',
    error instanceof Error ? error.message : 'Stripe could not be read.',
  );
}

/** Each metric owns its failure and deadline; the other metric remains useful. */
async function measured(work: (budget: FinancialBudget) => Promise<FinancialMetric>): Promise<FinancialMetric> {
  const budget = financialBudget();
  try {
    return await work(budget);
  } catch (error) {
    return failedMetric(error, budget);
  }
}

/** Nested items are independently paginated within the parent metric's budget. */
async function items(stripe: Stripe, subscription, budget: FinancialBudget) {
  const embedded = checkedPage(subscription.items, budget);
  if (!subscription.items.has_more) return embedded;
  if (!embedded.length) throw new IncompleteFinancial('Stripe returned an empty item page with more items.');
  return financialList(stripe, '/subscription_items', { subscription: subscription.id }, budget, embedded);
}

/** One subscription contributes only validated, supported fixed-price items. */
async function recurringAmounts(stripe: Stripe, subscription, budget: FinancialBudget, totals: Map<string, number>) {
  for (const item of await items(stripe, subscription, budget)) {
    const amount = fixedMonthly(item);
    if (amount) addMoney(totals, amount.currency, amount.amount);
  }
}

/** Active fixed per-unit month/year prices form this deliberately narrow monthly run rate. */
export async function mrr(stripe: Stripe): Promise<FinancialMetric> {
  return measured(async (budget) => {
    const totals = new Map<string, number>();
    for (const subscription of await financialList(stripe, '/subscriptions', { status: 'active' }, budget))
      await recurringAmounts(stripe, subscription, budget, totals);
    return { state: 'complete', values: moneyValues(totals), reason: '' };
  });
}

/** A missing final payment date never falls back to invoice creation time. */
function payment(row) {
  const paid = checkedInteger(row.status_transitions?.paid_at);
  if (!paid) throw new IncompleteFinancial('A paid invoice has no usable payment timestamp.');
  return { day: utcDay(paid * MS_PER_SECOND), currency: row.currency, amount: checkedInteger(row.amount_paid) };
}

/** Explicit date boundaries preserve currency throughout period arithmetic. */
function totalsBetween(rows, from: string, through: string) {
  const totals = new Map<string, number>();
  for (const row of rows.filter((row) => row.day >= from && row.day <= through))
    addMoney(totals, row.currency, row.amount);
  return moneyValues(totals);
}

/** Completed payment periods share a currency-safe aggregation model. */
function paidResult(rows, now: number): PaidInvoices {
  const periods = adminPeriods(now);
  return {
    state: 'complete',
    reason: '',
    values: totalsBetween(rows, periods.week.from, periods.week.to),
    previousValues: totalsBetween(rows, periods.previousWeek.from, periods.previousWeek.to),
    days: lastDays(now, DAYS_SHOWN).map((day) => ({ day, values: totalsBetween(rows, day, day) })),
  };
}

/** Full traversal is required even when older-created invoices were only recently paid. */
async function paidData(stripe: Stripe, now: number, budget: FinancialBudget): Promise<PaidInvoices> {
  const periods = adminPeriods(now);
  const rows = (await financialList(stripe, '/invoices', { status: 'paid' }, budget)).map(payment);
  return paidResult(
    rows.filter((row) => row.day >= periods.usage.from && row.day <= periods.usage.to),
    now,
  );
}

/** Invoice failure does not erase independently measured MRR. */
export async function payments(stripe: Stripe, now: number): Promise<PaidInvoices> {
  const result = await measured((budget) => paidData(stripe, now, budget));
  return result.state === 'complete' ? (result as PaidInvoices) : { ...result, previousValues: null, days: null };
}

/** An unconfigured deployment cannot fabricate a measured zero. */
function noFinance() {
  const unavailable = missing('not_configured', 'Stripe is not configured.');
  return [unavailable, { ...unavailable, previousValues: null, days: null }];
}

/** Two independent request chains bound finance without legacy false-zero scalar fields. */
export async function revenue(now = Date.now(), stripe: Stripe = stripeClient(stripeKey)) {
  const [mrrResult, paidInvoices] = hosted() ? await Promise.all([mrr(stripe), payments(stripe, now)]) : noFinance();
  return { generated_at: new Date(now).toISOString(), periods: adminPeriods(now), mrr: mrrResult, paidInvoices };
}
