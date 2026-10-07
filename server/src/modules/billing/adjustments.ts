/** Admin allowances, kept separate from Stripe's subscription and raw usage. */
import { PLANS, type PlanName } from './constants.ts';
import type { Standing } from './standing.ts';

/** Extra usage included in one billing period, in the meter's native units. */
export type Credits = Record<string, number>;

/** The optional allowance reader used by billing integrations. */
export type CreditReader = {
  /** Extra allowance for exactly this period; absent on older integrations. */
  creditsFor?(userId: string, since: string): Promise<Credits>;
};

/** Adds grants to the effective plan; complimentary plans stop at their allowance. */
export function includedFor(standing: Standing, credits: Credits = {}) {
  const plan = PLANS[standing.plan];
  return {
    ...plan,
    cloudSeconds: plan.cloudSeconds + (credits.cloud_seconds || 0),
    llmMicroUsd: plan.llmMicroUsd + (credits.hosted_llm_microusd || 0),
    overage: standing.status === 'admin' ? false : plan.overage,
  };
}

/** Only known plan names are usable as overrides. */
export const isPlan = (value: unknown): value is PlanName => typeof value === 'string' && Object.hasOwn(PLANS, value);
