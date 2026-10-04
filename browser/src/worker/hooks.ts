/**
 * The hooks the generated module calls around each step: pausing,
 * checkpoints, target checks, and the run events for each step.
 */
import type { Expect, Page } from '@playwright/test';
import type { Step } from '../workflow/index.ts';
import { checkTarget } from './target.ts';
import { REPLAY } from './constants.ts';
import { failureMessage } from './failure.ts';
import type { Run } from './types.ts';

/** Actions that change the website once dispatched: a failure after them has an unknown outcome. */
const INPUT_ACTIONS = ['click', 'double_click', 'press_key', 'upload_file', 'select_option', 'go_back', 'go_forward'];

/** What the generated module receives as its third argument. */
export interface StepHooks {
  /** Playwright's expect, so the module needs no import of its own. */
  expect: Expect;
  /** Each run tab by name. */
  pages: Map<string, Page>;
  /** False for a step that already passed (a rerun after a repair skips it). */
  shouldRun: (id: string) => boolean;
  /** Called before each step on the page it acts on. */
  beforeStep: (id: string, p: Page) => Promise<void>;
  /** A human checkpoint: waits until resumed. */
  checkpoint: () => Promise<void>;
  /** Called after each step that passed. */
  afterStep: (id: string) => Promise<void>;
  /** Called with a step's error before it propagates. */
  failedStep: (id: string, error: unknown) => Promise<void>;
}

/** The draft's step with this id. */
const stepOf = (run: Run, id: string | undefined): Step | undefined => run.draft.steps.find((s) => s.id === id);

/** Waits while paused (a breakpoint pauses unless the run itself just paused), then refuses if stopped. */
async function holdIfPaused(run: Run, step: Step): Promise<void> {
  const { state } = run;
  if (step.breakpoint && !state.requestedStop) state.paused = true;
  if (state.paused) {
    run.emit({ kind: 'step', stepId: step.id, status: 'paused' });
    await state.waitForWake();
    state.wake = undefined;
  }
  if (state.stopped) throw new Error('Run stopped');
}

/** The step is starting now: reset its input flag and clock, and report it running. */
function markRunning(run: Run, step: Step): void {
  Object.assign(run, { inputIssued: false, stepStarted: Date.now() });
  run.state.requestedStop = false;
  run.emit({ kind: 'step', stepId: step.id, status: 'running', action: step.action });
}

/**
 * Lets the page finish what its last load started. Script-heavy pages wire up
 * menus and widgets after the load event, so a click made the moment it fires
 * can land on a menu that does not open yet. A person waits; so does a step,
 * until the network is quiet, but never longer than SETTLE_MS (pages that poll
 * are never quiet).
 */
async function settle(run: Run, p: Page): Promise<void> {
  // What an input triggers (a grid's sort reload) starts a moment later, while the network still looks quiet.
  if (run.inputIssued) await p.waitForTimeout(REPLAY.SETTLE_GRACE_MS);
  await p.waitForLoadState('networkidle', { timeout: REPLAY.SETTLE_MS }).catch(() => {});
  // A run slowed down to watch: the pause the person chose, before each step.
  if (run.slowMo) await p.waitForTimeout(run.slowMo);
}

/** Before a step: pause if asked, report it running, let the page settle, check its target, note whether it sends input. */
async function beforeStep(run: Run, id: string, p: Page): Promise<void> {
  run.state.current = id;
  const step = stepOf(run, id) as Step;
  await holdIfPaused(run, step);
  await settle(run, p);
  markRunning(run, step);
  if (step.candidates.length) await checkTarget(run, step, p);
  // Once dispatched, a failed action may have changed the website.
  run.inputIssued = INPUT_ACTIONS.includes(step.action);
}

/** A human checkpoint: pause until the person has done it and resumes. */
async function checkpoint(run: Run): Promise<void> {
  const { state } = run;
  state.paused = true;
  const message = 'Complete the checkpoint using Take control, then resume.';
  run.emit({ kind: 'attention', stepId: state.current, status: 'paused', message });
  await state.waitForWake();
  if (state.stopped) throw new Error('Run stopped');
}

/** After a step: mark it done, report it, and pause when stepping or at the run-to step. */
async function afterStep(run: Run, id: string): Promise<void> {
  run.done.add(id);
  run.emit({ kind: 'step', stepId: id, status: 'passed', duration: Date.now() - run.stepStarted });
  const message = 'Screenshot omitted: safe masking cannot be guaranteed for this page.';
  if (run.evidence) run.emit({ kind: 'evidence', stepId: id, message });
  if (run.state.single || id === run.runTo) Object.assign(run.state, { paused: true, requestedStop: true });
}

/** A step failed: report it, unless the failure is a repair about to be retried. */
async function failedStep(run: Run, id: string, error: unknown): Promise<void> {
  if (run.repairSignal) return;
  const status = run.inputIssued ? 'outcome-unknown' : 'failed';
  const duration = Date.now() - run.stepStarted;
  run.emit({ kind: 'step', stepId: id, status, duration, message: failureMessage(error, stepOf(run, id)) });
}

/** The hooks object the generated module receives for this run. */
export function stepHooks(run: Run, expect: Expect): StepHooks {
  return {
    ...{ expect, pages: run.pages, shouldRun: (id: string) => !run.done.has(id) },
    ...{ beforeStep: (id: string, p: Page) => beforeStep(run, id, p), checkpoint: () => checkpoint(run) },
    ...{
      afterStep: (id: string) => afterStep(run, id),
      failedStep: (id: string, e: unknown) => failedStep(run, id, e),
    },
  };
}
