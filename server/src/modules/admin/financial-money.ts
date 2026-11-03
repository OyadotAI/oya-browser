/** Verified Stripe charge units and fixed-price arithmetic refuse unsupported financial inputs. */
import type { Money } from './financial-types.ts';
import { IncompleteFinancial } from './financial-pages.ts';
import { MONTHS_PER_YEAR } from './constants.ts';

/** Official list excluding UGX's two-decimal API exception: https://docs.stripe.com/currencies#special-cases */
const ZERO_DECIMAL = new Set('bif clp djf gnf jpy kmf krw mga pyg rwf vnd vuv xaf xof xpf'.split(' '));
/** API units remain unverified for these currencies; no ISO fallback is guessed. */
const AMBIGUOUS_UNITS = new Set('bhd jod kwd omr tnd'.split(' '));
/** ISK/UGX/HUF/TWD use two-decimal charge/invoice representation. */
const CHARGE_EXPONENT = 2;

/** Only supported API units enter an authoritative amount. */
export function currencyExponent(value: unknown) {
  if (typeof value !== 'string' || !/^[a-z]{3}$/i.test(value) || AMBIGUOUS_UNITS.has(value.toLowerCase()))
    throw new IncompleteFinancial('A reported currency has unsupported or unverified API units.');
  return ZERO_DECIMAL.has(value.toLowerCase()) ? 0 : CHARGE_EXPONENT;
}

/** Overflow is explicit rather than silently rounded, clamped or turned into Infinity. */
export function checkedAmount(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > Number.MAX_SAFE_INTEGER)
    throw new IncompleteFinancial('A financial amount is missing or exceeds supported arithmetic bounds.');
  return value;
}

/** Input source amounts and quantities are exact integers. */
export function checkedInteger(value: unknown): number {
  const amount = checkedAmount(value);
  if (!Number.isSafeInteger(amount))
    throw new IncompleteFinancial('A source financial amount or quantity is fractional.');
  return amount;
}

/** Values stay separated by currency through every arithmetic operation. */
export function addMoney(totals: Map<string, number>, currency: string, amount: number) {
  currencyExponent(currency);
  const key = currency.toLowerCase();
  totals.set(key, checkedAmount((totals.get(key) ?? 0) + checkedAmount(amount)));
}

/** Monthly equivalents are summed before one display rounding. */
export const moneyValues = (totals: Map<string, number>): Money[] =>
  [...totals].sort().map(([currency, amount]) => ({
    currency,
    amountMinor: Math.round(amount),
    minorUnitExponent: currencyExponent(currency),
  }));

/** A monthly amount preserves source currency and fractional yearly equivalents until aggregation. */
interface MonthlyAmount {
  /** Source currency. */ currency: string;
  /** Monthly API units before aggregation rounding. */ amount: number;
}

/** Licensed per-unit sources are validated before arithmetic. */
function fixedAmount(item): MonthlyAmount {
  const price = item?.price;
  if (price?.recurring?.usage_type !== 'licensed' || price.billing_scheme !== 'per_unit')
    throw new IncompleteFinancial('Fixed-price MRR includes an unsupported tiered or missing price.');
  const amount = checkedAmount(checkedInteger(price.unit_amount) * checkedInteger(item.quantity));
  currencyExponent(price.currency);
  return { currency: price.currency, amount: amount / intervalMonths(price.recurring) };
}

/** Metered charges are excluded; fixed inputs must fit the declared per-unit month/year model. */
export function fixedMonthly(item): MonthlyAmount | null {
  return item?.price?.recurring?.usage_type === 'metered' ? null : fixedAmount(item);
}

/** Unsupported day/week intervals never become an apparently exact monthly amount. */
function intervalMonths(recurring) {
  const count = checkedInteger(recurring.interval_count);
  if (!count || !['month', 'year'].includes(recurring.interval))
    throw new IncompleteFinancial('Fixed-price MRR supports positive month/year intervals only.');
  return checkedAmount(count * (recurring.interval === 'year' ? MONTHS_PER_YEAR : 1));
}
