/** One UTC period model keeps administrative totals, comparisons and chart dates aligned. */
import { ACTIVE_DAYS, DAYS_SHOWN, DAY_CHARS, MS_PER_DAY, WEEK_DAYS } from './constants.ts';

/** Public date labels always use UTC. */
export const utcDay = (time: number) => new Date(time).toISOString().slice(0, DAY_CHARS);

/** Today's unfinished interval is excluded from completed-period metrics. */
export const utcMidnight = (now: number) => Date.parse(`${utcDay(now)}T00:00:00.000Z`);

/** Inclusive date labels accompany an exclusive timestamp end for database filtering. */
export function completedWindow(end: number, days: number) {
  return { from: utcDay(end - days * MS_PER_DAY), to: utcDay(end - MS_PER_DAY), end: new Date(end).toISOString() };
}

/** Calendar-month leaders deliberately use a different scope from allowance-period customer usage. */
function calendarMonth(now: number) {
  const date = new Date(now);
  return {
    from: new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), 1)).toISOString(),
    to: new Date(now).toISOString(),
  };
}

/** Captured once per snapshot, these periods are returned with the data they describe. */
export function adminPeriods(now: number) {
  const end = utcMidnight(now);
  return {
    usage: completedWindow(end, DAYS_SHOWN),
    week: completedWindow(end, WEEK_DAYS),
    previousWeek: completedWindow(end - WEEK_DAYS * MS_PER_DAY, WEEK_DAYS),
    month: calendarMonth(now),
    reporting: { from: new Date(now - ACTIVE_DAYS * MS_PER_DAY).toISOString(), to: new Date(now).toISOString() },
  };
}
