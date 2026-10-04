/**
 * How a recorded step joins the list: normalized (or parked, turned off with
 * a note, when nothing identifies its element), a double-click in place of its
 * own two clicks, and a file picker's click turned off once its upload arrives.
 */
import { normalizeStep, TARGETED } from '../../workflow/index.ts';
import { DOUBLE_CLICK_MS, FILE_PICKER_CLICK_MS } from './constants.ts';
import type { RecordedStep } from './types.ts';

/**
 * A step with nothing to find its element by would stop the whole workflow from
 * saving or validating. It is kept, turned off, with the reason, for the person
 * to pick a target or delete.
 */
function parkUntargeted(step: RecordedStep): RecordedStep {
  // What could not be recorded (an unreachable frame, a drop) is kept, off, with its note: it never blocks a run.
  if (step.action.startsWith('unsupported_')) return { ...step, enabled: false };
  if (!TARGETED.includes(step.action) || step.candidates?.length) return step;
  return {
    ...step,
    enabled: false,
    captureIssue: 'Nothing identifies this element. Pick a target, or delete the step.',
  };
}

/** A recorded step normalized, or undefined when it is malformed: logged, never thrown, since a throw would stop the tab's later steps. */
export function safeStep(raw: RecordedStep): RecordedStep | undefined {
  try {
    return parkUntargeted(normalizeStep(raw) as RecordedStep);
  } catch (err) {
    console.error('[recording] dropped a step:', (err as Error).message);
    return undefined;
  }
}

/**
 * The click that opened a file picker, once its upload is recorded, is turned
 * off: the upload sets the file itself, and replaying the click would open the
 * operating system's dialog. It stays in the list for the person to turn back on.
 */
function disablePickerClick(steps: RecordedStep[]): void {
  const i = steps.length - 1;
  const last = steps[i];
  const prev = steps[i - 1];
  if (last?.action !== 'upload_file' || prev?.action !== 'click' || prev.tab !== last.tab) return;
  if ((last.t as number) - (prev.t as number) < FILE_PICKER_CLICK_MS) prev.enabled = false;
}

/**
 * A double-click reaches the page as two clicks and then the double-click.
 * Replayed, the two clicks would make it four, so they are dropped for it.
 */
function dropDoubleClickParts(steps: RecordedStep[], double: RecordedStep): void {
  const target = JSON.stringify(double.candidates);
  const part = (s: RecordedStep | undefined) =>
    s?.action === 'click' &&
    s.tab === double.tab &&
    (double.t ?? 0) - (s.t ?? 0) < DOUBLE_CLICK_MS &&
    JSON.stringify(s.candidates) === target;
  while (part(steps.at(-1))) steps.pop();
}

/** Puts the steps from index `from` on in time order, leaving the ones before it where they are. */
export function sortFrom(steps: RecordedStep[], from: number): void {
  const fresh = steps.splice(from).sort((a, b) => (a.t ?? 0) - (b.t ?? 0));
  steps.push(...fresh);
}

/** Adds a normalized step: a double-click replaces its own two clicks, and an upload turns off the click that opened its picker. */
export function appendStep(steps: RecordedStep[], step: RecordedStep): void {
  if (step.action === 'double_click') dropDoubleClickParts(steps, step);
  steps.push(step);
  disablePickerClick(steps);
}
