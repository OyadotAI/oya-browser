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
    overage: standing.status === 'admin' ? false : plan.overage,
    cloudSeconds: plan.cloudSeconds + (credits.cloud_seconds || 0),
    steps: plan.steps + (credits.agent_steps || 0),
    llmMicroUsd: hostedAllowance(standing) + (credits.hosted_llm_microusd || 0),
  };
}

/** Only known plan names are usable as overrides. */
export const isPlan = (value: unknown): value is PlanName => typeof value === 'string' && Object.hasOwn(PLANS, value);

/** Complimentary upgrades retain the Free hosted credit without enabling unbilled overage. */
function hostedAllowance(standing: Standing) {
  const included = PLANS[standing.plan].llmMicroUsd;
  return standing.status === 'admin' ? Math.max(included, PLANS.free.llmMicroUsd) : included;
}
