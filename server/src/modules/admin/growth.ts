/**
 * The admin page's growth numbers: one row per day of signups, people active,
 * usage, downloads, new installs and revenue, and how a stretch of days
 * compares with the one before (week over week, day over day). Pure functions
 * over rows, so they are tested without a database.
 */
import { FINGERPRINT_HEX_CHARS } from '../../platform/constants.ts';
import { DAY_CHARS, MONTH_DAYS, MS_PER_DAY, PERCENT, PERSON_PREFIX, WEEK_DAYS } from './constants.ts';

/** A row as storage hands it back. */
type Row = Record<string, any>;

/** What the daily series is worked out from. */
export type Sources = {
  /** Every account's profile. */
  profiles: Row[];
  /** Usage rows of the days shown, key rows and people's own rows. */
  usage: Row[];
  /** Every API key: its hash and owner. */
  keys: Row[];
  /** Download counts of the days shown. */
  downloads: Row[];
  /** Every self-hosted install. */
  installs: Row[];
  /** Payments as day and cents. */
  payments: Row[];
};

/** The counters a day row carries, each summed over that day. */
export const COUNTED = [
  ...['signups', 'active', 'agent_steps', 'cloud_seconds', 'browsers_started', 'commands'],
  ...['installers', 'update_checks', 'new_installs', 'revenue_cents'],
] as const;

/** One of the counters a day row carries. */
export type Counted = (typeof COUNTED)[number];

/** One day's counters. */
export type Day = Record<Counted, number> & {
  /** The UTC day, YYYY-MM-DD. */
  day: string;
};

/** Usage counters that mean someone used Oya in that hour. */
const ACTIVITY = ['commands', 'chat_requests', 'browsers_started', 'agent_steps', 'cloud_seconds'];

/** The UTC day of an ISO time. */
const dayOf = (iso: unknown) => String(iso || '').slice(0, DAY_CHARS);

/** Whether a usage row is a person's own row rather than one key's. */
const isPerson = (r: Row) => String(r.api_key).startsWith(PERSON_PREFIX);

/** Every UTC day from `days - 1` days before `now` to today, oldest first. */
export const lastDays = (now: number, days: number) =>
  Array.from({ length: days }, (_, i) => dayOf(new Date(now - (days - 1 - i) * MS_PER_DAY).toISOString()));

/** Who a usage row belongs to: the person of a `u:` row, or the owner of the key it fingerprints. */
export function ownerOf(keys: Row[]) {
  const byPrint = new Map(keys.map((k) => [String(k.key_hash).slice(0, FINGERPRINT_HEX_CHARS), String(k.user_id)]));
  return (apiKey: string) =>
    apiKey.startsWith(PERSON_PREFIX) ? apiKey.slice(PERSON_PREFIX.length) : byPrint.get(apiKey) || null;
}

/** The people active on each day: anyone whose key, or own row, did something that day. */
export function activePeople(usage: Row[], keys: Row[]) {
  const owner = ownerOf(keys);
  const days = new Map<string, Set<string>>();
  for (const r of usage) {
    const who = owner(String(r.api_key));
    if (!who || !ACTIVITY.some((f) => Number(r[f]) > 0)) continue;
    days.set(dayOf(r.hour), (days.get(dayOf(r.hour)) || new Set()).add(who));
  }
  return days;
}

/** `field` summed per day of `column`; each row counts once without a field. */
function sumByDay(rows: Row[], column: string, field?: string) {
  const sums = new Map<string, number>();
  for (const r of rows)
    sums.set(dayOf(r[column]), (sums.get(dayOf(r[column])) || 0) + (field ? Number(r[field]) || 0 : 1));
  return sums;
}

/** Download counts of one kind, per day. */
const downloadsOf = (rows: Row[], kind: string) =>
  sumByDay(
    rows.filter((r) => r.kind === kind),
    'day',
    'count',
  );

/** Usage counters per day. Steps and cloud time come from people's rows, the rest from key rows, so nothing counts twice. */
function usageColumns(usage: Row[]) {
  const [people, keyed] = [usage.filter(isPerson), usage.filter((r) => !isPerson(r))];
  const from = (rows: Row[], field: string) => sumByDay(rows, 'hour', field);
  return {
    ...{ agent_steps: from(people, 'agent_steps'), cloud_seconds: from(people, 'cloud_seconds') },
    ...{ browsers_started: from(keyed, 'browsers_started'), commands: from(keyed, 'commands') },
  };
}

/** Each counter's per-day sums. */
function columns(s: Sources, active: Map<string, Set<string>>): Record<Counted, Map<string, number>> {
  return {
    ...{ signups: sumByDay(s.profiles, 'created_at'), active: new Map([...active].map(([d, p]) => [d, p.size])) },
    ...usageColumns(s.usage),
    ...{ installers: downloadsOf(s.downloads, 'installer'), update_checks: downloadsOf(s.downloads, 'update_check') },
    ...{ new_installs: sumByDay(s.installs, 'first_seen'), revenue_cents: sumByDay(s.payments, 'day', 'cents') },
  };
}

/** One row per day of the last `days`, oldest first, with every counter (zero on a quiet day). */
export function daily(s: Sources, now: number, days: number, active = activePeople(s.usage, s.keys)): Day[] {
  const cols = columns(s, active);
  return lastDays(now, days).map(
    (day) => ({ day, ...Object.fromEntries(COUNTED.map((c) => [c, cols[c].get(day) || 0])) }) as Day,
  );
}

/** A stretch's total against the stretch before, with the change in percent (null when the one before was zero). */
export function compare(days: Day[], field: Counted, span: number) {
  const total = (rows: Day[]) => rows.reduce((n, d) => n + d[field], 0);
  const [now, before] = [total(days.slice(-span)), total(days.slice(-(span + span), -span))];
  return { now, before, change: before ? Math.round(((now - before) / before) * PERCENT) : null };
}

/** Every counter compared: the last 7 days against the 7 before, and yesterday (the last whole day) against the day before. */
export function trends(days: Day[]) {
  const each = (rows: Day[], span: number) => Object.fromEntries(COUNTED.map((c) => [c, compare(rows, c, span)]));
  return { week: each(days, WEEK_DAYS), day: each(days.slice(0, -1), 1) };
}

/** Distinct people active today, in the last 7 days and in the last 30. */
export function reach(active: Map<string, Set<string>>, now: number) {
  const over = (span: number) => new Set(lastDays(now, span).flatMap((d) => [...(active.get(d) || [])])).size;
  return { today: over(1), week: over(WEEK_DAYS), month: over(MONTH_DAYS) };
}
