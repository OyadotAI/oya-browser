/**
 * Where a person stands: which plan's allowances apply, and since when the
 * period's usage counts. Anyone without a paying subscription is on Free, and
 * Free's period is the calendar month (UTC).
 */
import { PAYING, PLANS, type PlanName } from './constants.ts';
import { isPlan } from './adjustments.ts';
import type { Subscription } from './repository.ts';

/** A person's plan for admission. */
export type Standing = {
  /** Who. */
  userId: string;
  /** The plan whose allowances apply. */
  plan: PlanName;
  /** The subscription's status, or null on Free with none. */
  status: string | null;
  /** When the period began, as an ISO time: usage from then on counts. */
  since: string;
};

/** The first instant of `now`'s UTC month. */
export function monthStart(now: number) {
  const d = new Date(now);
  return new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), 1)).toISOString();
}

/** Whether a stored plan name is one of the paid plans. */
const isPaid = (plan: unknown): plan is PlanName =>
  typeof plan === 'string' && plan !== 'free' && Object.hasOwn(PLANS, plan);

/** A paying or late subscription keeps its plan; anything else, or none, is Free for this month. */
export function standingOf(userId: string, row: Subscription | null, now: number): Standing {
  const keeps = row && isPaid(row.plan) && (PAYING.has(row.status) || row.status === 'past_due');
  if (isPlan(row?.admin_plan))
    return { userId, plan: row.admin_plan, status: 'admin', since: (keeps && row.period_start) || monthStart(now) };
  if (keeps) return { userId, plan: row.plan, status: row.status, since: row.period_start || monthStart(now) };
  return { userId, plan: 'free', status: row?.status ?? null, since: monthStart(now) };
}
