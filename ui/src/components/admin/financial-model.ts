/** Financial formatting respects explicit API units and never adds different currencies. */
import { DECIMAL_RADIX } from './constants';
import type { FinancialMetric, Money } from './financial-types';

/** Explicit source units override the locale's currency defaults, including Stripe exceptions. */
export function moneyText(money: Money) {
  return `${money.currency.toUpperCase()} ${new Intl.NumberFormat('en-US', {
    minimumFractionDigits: money.minorUnitExponent,
    maximumFractionDigits: money.minorUnitExponent,
  }).format(money.amountMinor / DECIMAL_RADIX ** money.minorUnitExponent)}`;
}

/** Successful empty money is zero without inventing an unreported currency. */
export function financialText(metric?: FinancialMetric | null) {
  if (!metric) return '—';
  if (metric.state !== 'complete')
    return { incomplete: 'Incomplete', unavailable: 'Unavailable', not_configured: 'Not configured' }[metric.state];
  return metric.values?.map(moneyText).join(' · ') || '0';
}
