/**
 * The billing page's numbers, shaped for display: pure functions, tested alone.
 */
import { SECONDS_PER_HOUR } from '@/lib/constants';
import { BYTES_PER_GB, CENT_DIGITS, CENTS_PER_DOLLAR, MICRO_USD_PER_DOLLAR, TENTHS } from './constants';
import type { Included, Used } from './use-plan';

/** One line of the usage breakdown. */
export interface UsageRow {
  /** What is counted. */
  label: string;
  /** How much was used, in the row's unit. */
  used: number;
  /** How much the plan includes, or null when the plan has none of it. */
  included: number | null;
  /** How used and included are said. */
  unit: string;
}

/** One decimal. */
const tenth = (n: number) => Math.round(n * TENTHS) / TENTHS;

/** Cloud time, in hours. */
const cloudRow = (used: Used, plan?: Included): UsageRow => ({
  label: 'Cloud browser time',
  used: tenth((used.cloud_seconds || 0) / SECONDS_PER_HOUR),
  included: plan ? tenth(plan.cloudSeconds / SECONDS_PER_HOUR) : null,
  unit: 'hours',
});

/** Residential proxy traffic, in GB. */
const proxyRow = (used: Used, plan?: Included): UsageRow => ({
  label: 'Residential proxy',
  used: tenth((used.residential_proxy_bytes || 0) / BYTES_PER_GB),
  included: plan?.proxyBytes ? tenth(plan.proxyBytes / BYTES_PER_GB) : null,
  unit: 'GB',
});

/** The hosted model's cost, in dollars. */
const modelRow = (used: Used, plan?: Included): UsageRow => ({
  label: 'AI model (hosted)',
  used: (used.hosted_llm_microusd || 0) / MICRO_USD_PER_DOLLAR,
  included: plan?.llmMicroUsd ? plan.llmMicroUsd / MICRO_USD_PER_DOLLAR : null,
  unit: 'USD',
});

/** The breakdown: each thing the plan counts, used against included. */
export function usageRows(used: Used = {}, plan?: Included): UsageRow[] {
  const steps = { label: 'Agent steps', used: used.agent_steps || 0, included: plan?.steps ?? null, unit: 'steps' };
  return [cloudRow(used, plan), steps, proxyRow(used, plan), modelRow(used, plan)];
}

/** How far into its allowance a row is, 0 to 1; 0 when the plan includes none. */
export const share = (row: UsageRow) => (row.included ? Math.min(row.used / row.included, 1) : 0);

/** A row's figure: dollars for money, a plain number otherwise. */
export const figure = (n: number, unit: string) => (unit === 'USD' ? `$${n.toFixed(CENT_DIGITS)}` : n.toLocaleString());

/** An amount in cents as money: $20.00, or 20.00 EUR. */
export const money = (cents = 0, currency = 'usd') =>
  currency === 'usd'
    ? `$${(cents / CENTS_PER_DOLLAR).toFixed(CENT_DIGITS)}`
    : `${(cents / CENTS_PER_DOLLAR).toFixed(CENT_DIGITS)} ${currency.toUpperCase()}`;

/** An ISO time as a short date, or a dash. */
export const dateOf = (iso?: string | null) =>
  iso ? new Date(iso).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' }) : '—';
