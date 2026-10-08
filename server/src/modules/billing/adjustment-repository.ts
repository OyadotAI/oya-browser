/** Durable admin plan overrides and append-only, retry-safe allowance grants. */
import { getConnection } from '../../platform/storage/index.ts';
import type { Credits } from './adjustments.ts';

/** The override row, separate from all webhook-managed fields. */
export async function overrideFor(userId: string) {
  const [row] = await getConnection().select('billing_overrides', { user_id: userId });
  return row || null;
}

/** Replaces only the override; null restores normal subscription access. */
export const saveOverride = (userId: string, plan: string | null, actor: string, reason: string) =>
  getConnection().upsert(
    'billing_overrides',
    [{ user_id: userId, plan, actor, reason, updated_at: new Date().toISOString() }],
    { update: true },
  );

/** Keeps each grant once, even if a client retries after losing the response. */
export const saveGrant = (row: Record<string, unknown>) => getConnection().upsert('billing_grants', [row]);

/** Finds a previous request so an id cannot silently be reused with different values. */
export async function grantById(id: string) {
  const [row] = await getConnection().select('billing_grants', { id });
  return row || null;
}

/** Grants belonging to exactly one billing period, for audit and display. */
export const grantsFor = (userId: string, since: string) =>
  getConnection().select('billing_grants', { user_id: userId, period_start: since });

/** Sums immutable grants rather than racing read-modify-write balance updates. */
export async function creditsFor(userId: string, since: string): Promise<Credits> {
  const rows = await grantsFor(userId, since);
  return {
    agent_steps: rows.reduce((sum, row) => sum + (Number(row.agent_steps) || 0), 0),
    cloud_seconds: rows.reduce((sum, row) => sum + Number(row.cloud_seconds), 0),
    hosted_llm_microusd: rows.reduce((sum, row) => sum + Number(row.hosted_llm_microusd), 0),
  };
}
