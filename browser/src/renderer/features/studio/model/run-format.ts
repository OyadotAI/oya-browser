/**
 * How a test run reads in the Run tab, as pure functions: its title and
 * summary, which controls work in its status, the timeline (each step's
 * latest event, nothing left "running" after the run ends), and whether a
 * repair can be applied.
 */
import { RendererConstants as C } from '../../../core/constants.ts';
import { RUN_TITLES, SAY, SHOWN_EVENT_KINDS, type RunCommand, RUN_CONTROLS } from './constants.ts';
import { actionName, isActive } from './studio-model.ts';
import { sameTarget } from './step-format.ts';
import type { Repair, Run, RunEvent, RunHistoryItem, Step, WorkspaceSnapshot } from './types.ts';

/** The heading: the status's title, the raw status if it has none, or the idle prompt. */
export function runTitle(run: Run | null | undefined): string {
  if (!run) return SAY.runIdle;
  return (Object.hasOwn(RUN_TITLES, run.status) && RUN_TITLES[run.status]) || run.status;
}

/** The line under the title: the error, what passed, or a warning. */
export function runSummary(run: Run | null | undefined): string {
  if (run?.error) return run.error;
  if (run?.status !== 'succeeded') return SAY.runWarning;
  return run.assertions ? `${run.assertions} assertions passed.` : SAY.noAssertions;
}

/** Whether run control `command` works in the run's status. */
export function controlEnabled(run: Run | null | undefined, command: RunCommand): boolean {
  const control = RUN_CONTROLS.find((c) => c.command === command);
  return (control?.statuses as readonly string[] | undefined)?.includes(run?.status ?? '') ?? false;
}

/** A step still "running" when the run is over ended with the run. */
function settled(event: RunEvent, run: Run): RunEvent {
  if (isActive(run) || event.status !== 'running') return event;
  return { ...event, status: run.status, message: runTitle(run) };
}

/** Each step's latest event (and every event without a step), most recent last, settled once the run is over. */
export function shownEvents(run: Run | null | undefined): RunEvent[] {
  if (!run) return [];
  const events = run.events.filter((e) => SHOWN_EVENT_KINDS.includes(e.kind));
  const seen = new Set<string>();
  const latest = events.reverse().filter((e) => !e.stepId || (!seen.has(e.stepId) && seen.add(e.stepId)));
  return latest
    .reverse()
    .slice(-C.RUN_EVENTS_SHOWN)
    .map((e) => settled(e, run));
}

/** An event's step number, or a dot when it has no step in the draft. */
export function eventNumber(event: RunEvent, steps: Step[]): string {
  const index = steps.findIndex((step) => step.id === event.stepId);
  return index < 0 ? '•' : String(index + 1).padStart(C.STEP_NUMBER_DIGITS, '0');
}

/** An event's message, or its step's action and status. */
export function eventText(event: RunEvent, steps: Step[]): string {
  const step = steps.find((s) => s.id === event.stepId);
  const status = event.status || event.kind;
  return event.message || (step ? `${actionName(step.action)} · ${status}` : status);
}

/** An event's duration, when timed. */
export const eventDuration = (event: RunEvent): string => (event.duration != null ? event.duration + ' ms' : '');

/** Whether the repair's step is in the workflow shown, with the run over and the target not yet first. */
export function canApply(s: WorkspaceSnapshot, repair: Repair): boolean {
  const step = s.draft.steps.find((item) => item.id === repair.stepId);
  if (!step || isActive(s.run) || s.run?.draftId !== s.draft.id) return false;
  return !sameTarget(step.candidates?.[0], repair.replacement);
}

/** A repair as its card says it. */
export const repairNote = (repair: Repair): string =>
  `${repair.original.kind} → ${repair.replacement.kind}. The target that worked in this run.`;

/** A previous run as its option says it. */
export const historyLabel = (item: RunHistoryItem): string =>
  item.name + ' · ' + new Date(item.updatedAt).toLocaleString();
