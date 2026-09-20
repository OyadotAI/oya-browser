/**
 * The validation worker, run in an Electron utility process: it replays a
 * draft's generated Playwright module against the run's tabs and reports
 * progress to the parent. The parent sends `start` once, then run controls.
 *
 * Behind it, in worker/: run (the replay loop), hooks (per-step pausing and
 * reporting), target (target checks and auto-heal), locate, evidence and
 * run-state (the controls).
 */
const { redact } = require('./diagnostics.cjs');
const { RunState } = require('./worker/run-state.cjs');
const { run } = require('./worker/run.cjs');

/** This run's controls and progress, shared with the replay. */
const state = new RunState();

/** Sends a message to the parent process. */
const tell = (message) => process.parentPort.postMessage(message);

/** Starts the run; a failure before any step reports the run finished. */
function start(data) {
  run(data, state, tell).catch((error) =>
    tell({ type: 'finished', status: state.stopped ? 'stopped' : 'failed', error: redact(error.message) }),
  );
}

/** Parent message type → what the worker does. */
const MESSAGES = { start, control: (data) => state.control(data.command) };

process.parentPort.on('message', ({ data }) => {
  if (Object.hasOwn(MESSAGES, data.type)) MESSAGES[data.type](data);
});
