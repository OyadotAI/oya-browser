/**
 * How the Routines pane words a routine and its runs: its schedule, what it is
 * doing in one line, its last run's pill, a run's summary and its answer as
 * HTML. Pure functions of the routine, the snapshot and the time `now`, so the
 * pane's clock redraws them by moving `now`.
 */
import { RendererConstants as C } from '../../../core/constants.ts';
import { escapeHtml, markdownToHtml } from '../../../core/markdown.ts';
import { ROUTINES_TEXT, RUNS_TEXT, SECONDS_PER_MINUTE } from './constants.ts';
import type { Phase, Routine, RoutineRun, RoutinesSnapshot, Schedule } from './types.ts';

/** Schedule kind → how a card names it. */
const SCHEDULE_LABEL: Readonly<Record<string, (s: Schedule) => string>> = {
  every: (s) => ROUTINES_TEXT.every(s.n ?? 1, s.unit ?? ''),
  daily: (s) => ROUTINES_TEXT.daily(s.at ?? ''),
};

/** How the routine's schedule reads; '' for a kind this app does not know. */
export function scheduleOf(routine: Routine): string {
  const kind = routine.schedule?.kind ?? '';
  return routine.schedule && Object.hasOwn(SCHEDULE_LABEL, kind) ? SCHEDULE_LABEL[kind](routine.schedule) : '';
}

/** What the routine is doing at `now`: running here or elsewhere, off, due, or scheduled. */
export function phaseOf(routine: Routine, running: string | null, now: number): Phase {
  if (routine.id === running) return 'here';
  if (routine.runs?.[0]?.status === 'running') return 'elsewhere';
  if (!routine.enabled || !routine.nextRunAt) return 'off';
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
  const seconds = Math.round((run.finishedAt - run.startedAt) / C.MS_PER_SECOND);
  const minutes = Math.floor(seconds / SECONDS_PER_MINUTE);
  return minutes ? `${minutes}m ${seconds % SECONDS_PER_MINUTE}s` : `${seconds}s`;
}

/** Phase → the card's one-line status. */
const PHASE_LINE: Readonly<Record<Phase, (r: Routine, snap: RoutinesSnapshot, now: number) => string>> = {
  here: (r, _s, now) =>
    ROUTINES_TEXT.runningHere(duration({ startedAt: r.runs?.[0]?.startedAt ?? now, finishedAt: now })),
  elsewhere: (r) => (r.target === 'cloud' ? ROUTINES_TEXT.runningCloud : ROUTINES_TEXT.runningElsewhere),
  off: (r) => `${scheduleOf(r)} · ${ROUTINES_TEXT.off}`,
  // A busy desktop holds back only its own routines; the cloud runs a cloud one regardless.
  due: (r, s) =>
    `${scheduleOf(r)} · ${s.busy && r.target !== 'cloud' ? ROUTINES_TEXT.waiting(s.busy) : ROUTINES_TEXT.dueNow}`,
  scheduled: (r, _s, now) => `${scheduleOf(r)} · ${ROUTINES_TEXT.next(when(r.nextRunAt ?? now, now))}`,
};

/** The routine's one-line status in `phase`. */
export const statusLine = (routine: Routine, phase: Phase, snap: RoutinesSnapshot, now: number): string =>
  PHASE_LINE[phase](routine, snap, now);

/** A status badge's words; a routine that never ran gets "Never run". */
export const badgeLabel = (status: string | undefined): string =>
  status && Object.hasOwn(RUNS_TEXT.status, status) ? RUNS_TEXT.status[status] : RUNS_TEXT.never;

/** A card's pill for its last run: how it ended and when, "Running", or "Never run". */
export function lastRunLabel(routine: Routine, now: number): string {
  const run = routine.runs?.[0];
  const label = badgeLabel(run?.status);
  return run && run.status !== 'running' ? `${label} · ${when(run.finishedAt || run.startedAt, now)}` : label;
}

/** "9:00 AM · 42s · 5 steps", and "another browser" when another browser than `browserId` ran it. */
export function runSummary(run: RoutineRun, browserId: string | undefined, now: number): string {
  const parts = [when(run.startedAt, now), duration(run)];
  if (run.steps?.length) parts.push(RUNS_TEXT.steps(run.steps.length));
  if (run.by && run.by !== browserId) parts.push(RUNS_TEXT.elsewhere);
  return parts.filter(Boolean).join(' · ');
}

/** The run's answer as HTML (escaped Markdown), or what it says when there is none. */
export const answerHtml = (run: RoutineRun): string =>
  run.result ? markdownToHtml(run.result) : escapeHtml(RUNS_TEXT.noAnswer[run.status] ?? '');

/** Whether the routine has finished runs, so Clear history has something to clear. */
export const hasFinishedRuns = (routine: Routine): boolean =>
  (routine.runs ?? []).some((run) => run.status !== 'running');
