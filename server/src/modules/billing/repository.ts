/**
 * Where billing keeps what it knows: a person's subscription as Stripe last
 * said it, and their usage for a period, summed from the person's own usage
 * rows (`u:<user id>`, see platform/usage.ts).
 */
import { getConnection } from '../../platform/storage/index.ts';
import { currentForPerson, personRow } from '../../platform/usage.ts';
import { HOUR_CHARS, METERS } from './constants.ts';

/** The table's name. */
const TABLE = 'subscriptions';

/** A person's subscription row. */
export type Subscription = Record<string, any> & {
  /** Who it is. */
  user_id: string;
};

/** A person's subscription, or null when they have never subscribed. */
export async function find(userId: string): Promise<Subscription | null> {
  const [row] = await getConnection().select(TABLE, { user_id: userId });
  return (row as Subscription) || null;
}

/** Writes what Stripe says of a person's subscription, leaving what was reported to it as it is. */
export async function saveStripe(userId: string, columns: Record<string, unknown>) {
  const set = { ...columns, updated_at: new Date().toISOString() };
  if ((await getConnection().update(TABLE, { user_id: userId }, set)) > 0) return;
  await getConnection().upsert(TABLE, [{ user_id: userId, ...set }]);
}

/** Records what of a person's usage has been reported to Stripe, and nothing else. */
export async function saveReported(userId: string, reported: Record<string, unknown>) {
  await getConnection().update(TABLE, { user_id: userId }, { reported, updated_at: new Date().toISOString() });
}

/** Every subscription. ponytail: reads the whole table, a status index if subscribers reach the thousands. */
export async function all(): Promise<Subscription[]> {
  return (await getConnection().select(TABLE, {})) as Subscription[];
}

/** The start of `iso`'s hour: usage is kept by the hour, so a period counts from its first whole hour. */
export const hourStart = (iso: string) => `${iso.slice(0, HOUR_CHARS)}:00:00.000Z`;

/**
 * A person's billed usage from `since` up to `until` (ISO times, taken to the
 * hour), summed over their hourly rows. The current hour is read from memory,
 * so what was used in the last minute, not yet written, counts too.
 */
export async function usageSince(userId: string, since: string, until?: string): Promise<Record<string, number>> {
  const live = currentForPerson(userId);
  const rows = await getConnection().select('usage', { api_key: personRow(userId), hour: { gte: hourStart(since) } });
  const stored = rows.filter((r) => String(r.hour) !== live.hour && (!until || String(r.hour) < hourStart(until)));
  const counted = !until || live.hour < hourStart(until) ? [...stored, live.counters] : stored;
  return Object.fromEntries(
    METERS.map((m) => [m.field, counted.reduce((sum, r) => sum + (Number(r[m.field]) || 0), 0)]),
  );
}
