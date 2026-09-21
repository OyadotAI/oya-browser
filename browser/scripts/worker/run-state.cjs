/**
 * The validation worker's run controls: pause, resume, single-step and stop,
 * and the wait a paused step sits in until one of them arrives.
 */

/** What the parent can ask a running validation to do. */
const CONTROLS = {
  stop: (state) => state.stop(),
  pause: (state) => (state.paused = true),
  resume: (state) => state.resume(false),
  step: (state) => state.resume(true),
};

/** Shared by the controls and the run: whether it is paused, stopping, or stepping. */
class RunState {
  /** A run that is neither paused nor stopped. */
  constructor() {
    /** Waiting at the next step until resumed. */
    this.paused = false;
    /** A stop was asked for. */
    this.stopped = false;
    /** Pause again after the next step. */
    this.single = false;
    /** Resolves the wait a paused step is in. */
    this.wake = undefined;
    /** The Playwright connection, closed on stop. */
    this.browser = undefined;
    /** The step being run, for evidence and the final report. */
    this.current = undefined;
    /** The pause was the run's own (a breakpoint, a step or runTo), not a person's. */
    this.requestedStop = false;
  }

  /** Applies a run control; an unknown one is ignored. */
  control(command) {
    if (Object.hasOwn(CONTROLS, command)) CONTROLS[command](this);
  }

  /** Stops the run: wakes a paused step and closes the browser connection. */
  stop() {
    this.stopped = true;
    this.wake?.();
    this.browser?.close().catch(() => {});
  }

  /** Continues, pausing again after one step when `single`. */
  resume(single) {
    this.single = single;
    this.paused = false;
    this.wake?.();
  }

  /** Resolves when the run is resumed or stopped. */
  waitForWake() {
    return new Promise((resolve) => (this.wake = resolve));
  }
}

module.exports = { RunState };
