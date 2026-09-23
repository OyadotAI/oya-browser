/**
 * Routines: saved prompts the agent runs on a schedule, "every N minutes or
 * hours" or "daily at HH:MM", while the app is open and connected. They live in
 * config.json beside the other settings. A run is an ordinary Ask (control is
 * lent to the agent, Stop stops it), one at a time, never over a chat or a
 * recording. A run missed while the app was closed fires once, on the next tick.
 * Each routine keeps its last ROUTINE_RUNS_KEPT runs: when, how it ended, the
 * steps the agent took and what it answered.
 *
 * ponytail: the history lives in config.json, capped per routine; move it to its
 * own file if routines or their answers grow past what a settings file should hold.
 */
const crypto = require('crypto');
const { sendChat, STOPPED } = require('./ipc/dev.cjs');
const {
  ROUTINE_TICK_MS,
  ROUTINE_UNIT_MS,
  ROUTINE_MAX_EVERY,
  ROUTINE_MAX_NAME,
  ROUTINE_MAX_PROMPT,
  ROUTINE_RESULT_CHARS,
  ROUTINE_RUNS_KEPT,
  ROUTINE_STEPS_KEPT,
} = require('./constants.cjs');

/** A daily time, 24-hour HH:MM. */
const DAILY_TIME = /^([01]\d|2[0-3]):[0-5]\d$/;

/** The first moment after `since` that today's (or tomorrow's) HH:MM comes round. */
function nextDaily(at, since) {
  const [hours, minutes] = at.split(':').map(Number);
  const next = new Date(since);
  next.setHours(hours, minutes, 0, 0);
  if (next.getTime() <= since) next.setDate(next.getDate() + 1);
  return next.getTime();
}

/** Schedule kind → when a routine last run at `since` is next due. */
const NEXT_RUN = {
  every: ({ n, unit }, since) => since + n * ROUTINE_UNIT_MS[unit],
  daily: ({ at }, since) => nextDaily(at, since),
};

/** Schedule kind → whether a schedule from the renderer is well formed. */
const VALID_SCHEDULE = {
  every: ({ n, unit }) =>
    Number.isInteger(n) && n >= 1 && n <= ROUTINE_MAX_EVERY && Object.hasOwn(ROUTINE_UNIT_MS, unit),
  daily: ({ at }) => typeof at === 'string' && DAILY_TIME.test(at),
};

/** When `routine` is next due, counted from its last run (or its creation). */
function nextRunAt(routine) {
  const { schedule } = routine;
  return NEXT_RUN[schedule.kind](schedule, routine.lastRunAt ?? routine.createdAt);
}

/** Whether `routine` should run at `now`. */
const isDue = (routine, now) => routine.enabled && nextRunAt(routine) <= now;

/** A text of 1 to `max` characters, trimmed. */
const boundedText = (value, max) => typeof value === 'string' && value.trim().length > 0 && value.length <= max;

/** Whether a routine the renderer sent is one this scheduler can keep. */
function validRoutine(input) {
  const schedule = input?.schedule;
  if (!boundedText(input?.name, ROUTINE_MAX_NAME) || !boundedText(input?.prompt, ROUTINE_MAX_PROMPT)) return false;
  return !!schedule && Object.hasOwn(VALID_SCHEDULE, schedule.kind) && VALID_SCHEDULE[schedule.kind](schedule);
}

/** Just the fields a schedule of its kind has, so nothing else from the renderer is stored. */
function cleanSchedule(schedule) {
  return schedule.kind === 'every'
    ? { kind: 'every', n: schedule.n, unit: schedule.unit }
    : { kind: 'daily', at: schedule.at };
}

/** How a run ended, from the agent's answer. */
function statusOf(answer) {
  if (answer?.error === STOPPED) return 'stopped';
  return answer?.error || answer?.failed ? 'failed' : 'done';
}

/** A finished run's record: when it ended, how, the steps it took and what it answered. */
function finishedRun(answer) {
  const text = answer?.error && answer.error !== STOPPED ? `Error: ${answer.error}` : answer?.text || '';
  const steps = (answer?.toolCalls || []).map((call) => call.name).slice(0, ROUTINE_STEPS_KEPT);
  return { finishedAt: Date.now(), status: statusOf(answer), result: text.slice(0, ROUTINE_RESULT_CHARS), steps };
}

/** `routine` with `run` first in its history, the oldest past the cap dropped. */
const withRun = (routine, run) => ({ ...routine, runs: [run, ...(routine.runs || [])].slice(0, ROUTINE_RUNS_KEPT) });

/** `routine` with its run `runId` changed by `changes`. */
const withRunChanged = (routine, runId, changes) => ({
  ...routine,
  runs: (routine.runs || []).map((run) => (run.id === runId ? { ...run, ...changes } : run)),
});

/** A run the app quit in the middle of: it will never finish, so it says it was interrupted. */
const interrupted = (routine) => ({
  ...routine,
  runs: (routine.runs || []).map((run) => (run.status === 'running' ? { ...run, status: 'interrupted' } : run)),
});

/** The routine to keep for a valid `input`: `old`'s id and history (or new ones), the input's settings. */
function routineFrom(input, old) {
  return {
    ...(old || { id: crypto.randomUUID(), createdAt: Date.now() }),
    name: input.name.trim(),
    prompt: input.prompt.trim(),
    schedule: cleanSchedule(input.schedule),
    enabled: input.enabled !== false,
  };
}

/** The saved routines and the timer that runs them. */
class Routines {
  /** `ctx` is the main-process context (see main.js); `run` asks the agent (sendChat, faked in tests). */
  constructor(ctx, run = sendChat) {
    /** The main-process context. */
    this.ctx = ctx;
    /** Asks the agent: `(ctx, event, messages)` → its answer. */
    this.run = run;
    /** The id of the routine running now, or null. */
    this.running = null;
  }

  /** The saved routines, oldest first. */
  list() {
    return this.ctx.config.values.routines || [];
  }

  /** The list, each with when it next runs (null when paused), and the one running, as the Routines pane shows them. */
  snapshot() {
    const routines = this.list().map((r) => ({ ...r, nextRunAt: r.enabled ? nextRunAt(r) : null }));
    return { routines, running: this.running };
  }

  /** Saves `routines` and tells the shell. */
  store(routines) {
    this.ctx.config.values.routines = routines;
    this.ctx.config.save();
    this.ctx.shell.send('routines-changed', this.snapshot());
  }

  /** Marks runs the last quit cut short, then checks for a due routine every tick, for as long as the app runs. */
  start() {
    if (this.list().some((r) => r.runs?.some((run) => run.status === 'running')))
      this.store(this.list().map(interrupted));
    setInterval(() => this.tick(), ROUTINE_TICK_MS).unref?.();
  }

  /** Adds a routine, or replaces the one with its id; its run history is kept. */
  save(input) {
    if (!validRoutine(input)) throw new Error('A routine needs a name, a prompt and a valid schedule.');
    const old = this.list().find((r) => r.id === input.id);
    const routine = routineFrom(input, old);
    this.store(old ? this.list().map((r) => (r.id === old.id ? routine : r)) : [...this.list(), routine]);
    return this.snapshot();
  }

  /** Forgets the routine with this id. */
  remove(id) {
    this.store(this.list().filter((r) => r.id !== id));
    return this.snapshot();
  }

  /** Replaces one routine with `change(routine)` and saves. */
  update(id, change) {
    this.store(this.list().map((r) => (r.id === id ? change(r) : r)));
  }

  /** Whether a routine may start now: connected, and nothing else is driving the browser. */
  idle() {
    return this.ctx.socket.ready && !this.ctx.chatAbort && !this.ctx.recorder.recording && !this.running;
  }

  /** Runs the first due routine, if the browser is free. */
  async tick(now = Date.now()) {
    const due = this.list().find((r) => isDue(r, now));
    if (due) await this.runNow(due.id);
  }

  /** Runs one routine now, when the browser is free; answers the list after. */
  async runNow(id) {
    const routine = this.list().find((r) => r.id === id);
    if (routine && this.idle()) await this.execute(routine);
    return this.snapshot();
  }

  /** Asks the agent the routine's prompt and records the run in its history; its next run counts from this start. */
  async execute(routine) {
    const run = { id: crypto.randomUUID(), startedAt: Date.now(), status: 'running' };
    this.running = routine.id;
    this.update(routine.id, (r) => ({ ...withRun(r, run), lastRunAt: run.startedAt }));
    const ask = [{ role: 'user', content: routine.prompt }];
    const answer = await this.run(this.ctx, null, ask).catch((e) => ({ error: e.message }));
    this.running = null;
    this.update(routine.id, (r) => withRunChanged(r, run.id, finishedRun(answer)));
  }
}

module.exports = { Routines, isDue, validRoutine };
