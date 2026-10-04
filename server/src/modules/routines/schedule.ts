/**
 * When a routine runs next, as the server works it out for cloud routines. A
 * desktop works out its own the same way in its local time; the server has no
 * local time worth using, so a daily routine's time is read in its `tz`.
 */
import type { Routine, Schedule } from './rules.ts';
import { ROUTINE_RUN_LEASE_MS, ROUTINE_UNIT_MS } from './constants.ts';

/** The wall-clock parts of `t` in `tz`, as a formatter gives them. */
const WALL = (tz: string) =>
  new Intl.DateTimeFormat('en-US', {
    timeZone: tz,
    hourCycle: 'h23',
    ...{ year: 'numeric', month: 'numeric', day: 'numeric', hour: 'numeric', minute: 'numeric' },
  });

/** How far `tz`'s clock is ahead of UTC at `t`, in ms. */
function offsetAt(tz: string, t: number): number {
  const p = Object.fromEntries(
    WALL(tz)
      .formatToParts(t)
      .map((x) => [x.type, Number(x.value)]),
  );
  return Date.UTC(p.year, p.month - 1, p.day, p.hour, p.minute) - (t - (t % ROUTINE_UNIT_MS.minutes));
}

/** The instant a wall time (month from 0, day may overflow) happens in `tz`. */
function instantOf(tz: string, y: number, mo: number, d: number, at: string): number {
  const [h, m] = at.split(':').map(Number);
  const wall = Date.UTC(y, mo, d, h, m);
  return wall - offsetAt(tz, wall - offsetAt(tz, wall));
}

/** The first `at` in `tz` after `since`: today's, or else tomorrow's. */
export function nextDaily(at: string, tz: string, since: number): number {
  const local = new Date(since + offsetAt(tz, since));
  const day = (plus: number) =>
    instantOf(tz, local.getUTCFullYear(), local.getUTCMonth(), local.getUTCDate() + plus, at);
  return day(0) > since ? day(0) : day(1);
}

/** Schedule kind → the next run after `since`. */
const NEXT_RUN: Record<string, (s: any, since: number, tz: string) => number> = {
  every: ({ n, unit }, since) => since + n * ROUTINE_UNIT_MS[unit],
  daily: ({ at }, since, tz) => nextDaily(at, tz, since),
};

/** When the routine runs next: counted from its last run, or from when it was made. */
export function nextRunAt(r: Routine): number {
  const schedule: Schedule = r.schedule;
  if (!Object.hasOwn(NEXT_RUN, schedule.kind)) return Infinity;
  return NEXT_RUN[schedule.kind](schedule, r.lastRunAt ?? r.createdAt, r.tz || 'UTC');
}

/** Whether a run is still going: marked running, and inside its lease. */
const running = (r: Routine, now: number) =>
  r.runs.some((run) => run.status === 'running' && now - run.startedAt < ROUTINE_RUN_LEASE_MS);

/** Whether the routine should start now: on, due, and not already running. */
export const isDue = (r: Routine, now: number) => r.enabled && nextRunAt(r) <= now && !running(r, now);
