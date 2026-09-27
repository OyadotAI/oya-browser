/**
 * The admin overview, worked out from rows: pure functions, so the page's
 * numbers are tested without a database.
 */
import { ACTIVE_DAYS, DAY_CHARS, MS_PER_DAY, TOP_USERS } from './constants.ts';

/** How a person's own usage row is keyed: `u:<user id>` (platform/usage.ts personRow). */
const PERSON_PREFIX = 'u:';

/** A row as storage hands it back. */
type Row = Record<string, any>;

/** The UTC day of an ISO time. */
export const dayOf = (iso: unknown) => String(iso || '').slice(0, DAY_CHARS);

/** The UTC day `days` before `now`. */
export const daysAgo = (now: number, days: number) =>
  new Date(now - days * MS_PER_DAY).toISOString().slice(0, DAY_CHARS);

/** How many rows fall on each day, oldest first. */
export function perDay(rows: Row[], column: string, from: string) {
  const counts = new Map<string, number>();
  for (const r of rows)
    if (dayOf(r[column]) >= from) counts.set(dayOf(r[column]), (counts.get(dayOf(r[column])) || 0) + 1);
  return [...counts.entries()].sort().map(([day, count]) => ({ day, count }));
}

/** Paying people by plan, and how many are late. */
export function plans(subscriptions: Row[]) {
  const paying = subscriptions.filter(
    (s) => s.plan !== 'free' && ['active', 'trialing', 'past_due'].includes(s.status),
  );
  const byPlan: Record<string, number> = {};
  for (const s of paying) byPlan[s.plan] = (byPlan[s.plan] || 0) + 1;
  return { byPlan, pastDue: paying.filter((s) => s.status === 'past_due').length };
}

/** Each person's usage summed from their own `u:` rows. */
export function perPerson(rows: Row[]) {
  const totals = new Map<string, Row>();
  for (const r of rows.filter((r) => String(r.api_key).startsWith(PERSON_PREFIX))) {
    const id = String(r.api_key).slice(PERSON_PREFIX.length);
    const t = totals.get(id) || { userId: id, cloud_seconds: 0, agent_steps: 0, hosted_llm_microusd: 0 };
    for (const f of ['cloud_seconds', 'agent_steps', 'hosted_llm_microusd']) t[f] += Number(r[f]) || 0;
    totals.set(id, t);
  }
  return [...totals.values()];
}

/** The heaviest people by `field`, with their email. */
export function top(people: Row[], field: string, emails: Map<string, string>) {
  const sorted = people.filter((p) => p[field] > 0).sort((a, b) => b[field] - a[field]);
  return sorted.slice(0, TOP_USERS).map((p) => ({ ...p, email: emails.get(p.userId) || null }));
}

/** Installs: how many, how many pinged lately, and how many run past the free cap without a license. */
export function installSummary(installs: Row[], now: number, freeCap: number) {
  const since = new Date(now - ACTIVE_DAYS * MS_PER_DAY).toISOString();
  const active = installs.filter((i) => String(i.last_seen) >= since);
  const overCap = active.filter((i) => !i.license_id && Number(i.peak_cloud) > freeCap).length;
  return { total: installs.length, active: active.length, overCap };
}

/** Browsers connected now: in all, in the cloud, and by provider. */
export function fleet(browsers: Row[], cloud: Set<string>) {
  const byProvider: Record<string, number> = {};
  for (const b of browsers) byProvider[b.provider || 'unknown'] = (byProvider[b.provider || 'unknown'] || 0) + 1;
  return { total: browsers.length, cloud: browsers.filter((b) => cloud.has(b.provider)).length, byProvider };
}
