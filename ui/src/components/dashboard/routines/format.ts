/**
 * How the Routines tab words a routine and its runs, ported from the desktop's
 * routine-format.ts: its schedule, its phase, its last run's badge and a run's
 * summary. Pure functions of the routine and `now`, so the tab's clock redraws
 * them by moving `now`.
 */
import { MS_PER_SECOND, ROUTINES_TEXT, RUN_LEASE_MS, RUNS_TEXT, SECONDS_PER_MINUTE } from './constants';
import type { Phase, Routine, RoutineRun, Schedule } from './types';

/** Schedule kind → how a row names it. */
const SCHEDULE_LABEL: Readonly<Record<string, (s: Schedule) => string>> = {
  every: (s) => ROUTINES_TEXT.every(s.n ?? 1, s.unit ?? ''),
  daily: (s) => ROUTINES_TEXT.daily(s.at ?? ''),
};

/** How the routine's schedule reads; '' for a kind the console does not know. */
export function scheduleOf(routine: Routine): string {
  const kind = routine.schedule?.kind ?? '';
  return Object.hasOwn(SCHEDULE_LABEL, kind) ? SCHEDULE_LABEL[kind](routine.schedule) : '';
}

/** Whether a run is still going at `now`: marked running and inside the server's lease. */
const isLive = (run: RoutineRun | undefined, now: number) =>
  run?.status === 'running' && now - run.startedAt < RUN_LEASE_MS;

/** What the routine is doing at `now`: running, off, due, or waiting for its next run. */
export function phaseOf(routine: Routine, now: number): Phase {
  if (isLive(routine.runs[0], now)) return 'running';
  if (!routine.enabled || routine.nextRunAt === null) return 'off';
  return routine.nextRunAt <= now ? 'due' : 'scheduled';
}

/** A moment, as its time on `now`'s day, or its date and time on another day. */
export function when(ts: number, now: number): string {
  const date = new Date(ts);
  if (date.toDateString() === new Date(now).toDateString())
    return date.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
  return date.toLocaleString([], { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });
}

/** How long a finished run took, as "42s" or "3m 5s"; '' while it runs. */
export function duration(run: Pick<RoutineRun, 'startedAt' | 'finishedAt'>): string {
  if (!run.finishedAt) return '';
  const seconds = Math.round((run.finishedAt - run.startedAt) / MS_PER_SECOND);
  const minutes = Math.floor(seconds / SECONDS_PER_MINUTE);
  return minutes ? `${minutes}m ${seconds % SECONDS_PER_MINUTE}s` : `${seconds}s`;
}

/** A status badge's words; a routine that never ran gets "Never run". */
export const badgeLabel = (status: string | undefined): string =>
  status && Object.hasOwn(RUNS_TEXT.status, status) ? RUNS_TEXT.status[status] : RUNS_TEXT.never;

/** A row's badge for its last run: how it ended and when, "Running", or "Never run". */
export function lastRunLabel(routine: Routine, now: number): string {
  const run = routine.runs[0];
  const label = badgeLabel(run?.status);
  return run && run.status !== 'running' ? `${label} · ${when(run.finishedAt || run.startedAt, now)}` : label;
}

/** "9:00 AM · 42s · 5 steps". */
export function runSummary(run: RoutineRun, now: number): string {
  const parts = [when(run.startedAt, now), duration(run)];
  if (run.steps?.length) parts.push(RUNS_TEXT.steps(run.steps.length));
  return parts.filter(Boolean).join(' · ');
}

/** The Next run column's words, by phase. */
const NEXT_LINE: Readonly<Record<Phase, (r: Routine, now: number) => string>> = {
  running: (r) => (r.target === 'cloud' ? ROUTINES_TEXT.runningCloud : ROUTINES_TEXT.runningDesktop),
  off: () => ROUTINES_TEXT.off,
  due: () => ROUTINES_TEXT.dueNow,
  scheduled: (r, now) => when(r.nextRunAt ?? now, now),
};

/** What the Next run column says for the routine at `now`. */
export const nextLine = (routine: Routine, now: number): string => NEXT_LINE[phaseOf(routine, now)](routine, now);

/** Whether the routine has finished runs, so Clear history has something to clear. */
export const hasFinishedRuns = (routine: Routine): boolean => routine.runs.some((run) => run.status !== 'running');

/** The answer a run shows: its own, or what it says when it has none. */
export const answerOf = (run: RoutineRun): string => run.result || RUNS_TEXT.noAnswer[run.status] || '';
