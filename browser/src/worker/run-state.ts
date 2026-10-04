/**
 * The validation worker's run controls: pause, resume, single-step and stop,
 * and the wait a paused step sits in until one of them arrives.
 */
import type { Browser } from '@playwright/test';

/** What the parent can ask a running validation to do. */
const CONTROLS: Record<string, (state: RunState) => void> = {
  stop: (state) => state.stop(),
  pause: (state) => (state.paused = true),
  resume: (state) => state.resume(false),
  step: (state) => state.resume(true),
};

/** Shared by the controls and the run: whether it is paused, stopping, or stepping. */
export class RunState {
  /** Waiting at the next step until resumed. */
  paused = false;
  /** A stop was asked for. */
  stopped = false;
  /** Pause again after the next step. */
  single = false;
  /** Resolves the wait a paused step is in. */
  wake: (() => void) | undefined = undefined;
  /** The Playwright connection, closed on stop. */
  browser: Browser | undefined = undefined;
  /** The step being run, for evidence and the final report. */
  current: string | undefined = undefined;
  /** The pause was the run's own (a breakpoint, a step or runTo), not a person's. */
  requestedStop = false;

  /** Applies a run control; an unknown one is ignored. */
  control(command: string): void {
    if (Object.hasOwn(CONTROLS, command)) CONTROLS[command](this);
  }

  /** Stops the run: wakes a paused step and closes the browser connection. */
  stop(): void {
    this.stopped = true;
    this.wake?.();
    this.browser?.close().catch(() => {});
  }

  /** Continues, pausing again after one step when `single`. */
  resume(single: boolean): void {
    this.single = single;
    this.paused = false;
    this.wake?.();
  }

  /** Resolves when the run is resumed or stopped. */
  waitForWake(): Promise<void> {
    return new Promise((resolve) => (this.wake = resolve));
  }
}
