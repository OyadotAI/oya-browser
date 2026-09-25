/**
 * Routines: saved prompts the agent runs on a schedule, "every N minutes or
 * hours" or "daily at HH:MM". They belong to the project and live on the
 * server with their history, so every desktop on the project shows the same
 * list; this app runs them while it is open and connected. Before a run, the
 * app claims it from the server, which gives each due run to exactly one
 * desktop. A run is an ordinary Ask (control is lent to the agent), one at a
 * time, never over a chat or a recording; a run missed while every desktop was
 * closed fires once, on the next tick. Routines this app kept locally before
 * they moved to the server are handed to the project on the first connect.
 *
 * ponytail: a run's ending that cannot reach the server (offline mid-run) is
 * not retried; the server's lease marks it interrupted. Queue it if that matters.
 */
const crypto = require('crypto');
const { sendChat, STOPPED } = require('./ipc/dev.cjs');
const { canCallServer, getFromApi, sendToApi } = require('./connection/server-api.cjs');
const { ROUTINE_TICK_MS, ROUTINE_UNIT_MS, ROUTINE_RESULT_CHARS, ROUTINE_STEPS_KEPT } = require('./constants.cjs');

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

/** When `routine` is next due, counted from its last run (or its creation); null for a schedule this app does not know. */
function nextRunAt(routine) {
  const { schedule } = routine;
  if (!Object.hasOwn(NEXT_RUN, schedule?.kind)) return null;
  return NEXT_RUN[schedule.kind](schedule, routine.lastRunAt ?? routine.createdAt);
}

/** Whether `routine` should run at `now`: on, due, and not running anywhere. */
const isDue = (routine, now) =>
  routine.enabled && nextRunAt(routine) !== null && nextRunAt(routine) <= now && !runningRun(routine);

/** The routine's run in progress, on any desktop, or undefined. */
const runningRun = (routine) => (routine.runs || []).find((run) => run.status === 'running');

/** How a run ended, from the agent's answer. */
function runStatusOf(answer) {
  if (answer?.error === STOPPED) return 'stopped';
  return answer?.error || answer?.failed ? 'failed' : 'done';
}

/** A finished run's ending, as the server records it: how, what it answered, and the steps it took. */
function endingOf(answer) {
  const text = answer?.error && answer.error !== STOPPED ? `Error: ${answer.error}` : answer?.text || '';
  const steps = (answer?.toolCalls || []).map((call) => call.name).slice(0, ROUTINE_STEPS_KEPT);
  return { status: runStatusOf(answer), result: text.slice(0, ROUTINE_RESULT_CHARS), steps };
}

/** What Run now and the pane say when this app cannot run a routine now, or '' when it can. */
function busyReason(ctx, running) {
  if (!canCallServer(ctx)) return 'Connect to Oya to run routines.';
  if (running) return 'Another routine is running. Stop it first.';
  if (ctx.recorder.recording) return 'Finish recording first.';
  return ctx.chatAbort ? 'The agent is busy with an Ask. Stop it first.' : '';
}

/** The project's routines, as this app last read them, and the timer that runs the due ones. */
class Routines {
  /** `ctx` is the main-process context (see main.js); `run` asks the agent (sendChat, faked in tests). */
  constructor(ctx, run = sendChat) {
    Object.assign(this, { ctx, run });
    /** The project's routines, as last read from the server. */
    this.list = [];
    /** The routine and run this app is running now, or null. */
    this.current = null;
    /** Why the list could not be read or changed last, or ''. */
    this.error = '';
  }

  /** The list with each routine's next run, and what this app is doing, as the Routines pane shows it. */
  snapshot() {
    const routines = this.list.map((r) => ({ ...r, nextRunAt: r.enabled ? nextRunAt(r) : null }));
    const running = this.current?.routineId || null;
    const state = { online: canCallServer(this.ctx), error: this.error, busy: busyReason(this.ctx, running) };
    return { routines, running, browserId: this.ctx.socket.browserId, ...state };
  }

  /** Tells the pane what changed. */
  publish() {
    this.ctx.shell.send('routines-changed', this.snapshot());
    return this.snapshot();
  }

  /** Checks for a due routine every tick, for as long as the app runs. */
  start() {
    setInterval(() => void this.tick(), ROUTINE_TICK_MS).unref?.();
  }

  /** Re-reads the project's routines (handing over local ones first); answers the snapshot. */
  async refresh() {
    if (!canCallServer(this.ctx)) return this.publish();
    this.error = await this.load().then(
      () => '',
      (e) => e.message,
    );
    return this.publish();
  }

  /** Hands over local routines, reads the project's, and closes out runs a quit left behind. */
  async load() {
    await this.handOverLocal();
    this.list = (await getFromApi(this.ctx, 'routines')).routines || [];
    await this.markInterrupted();
  }

  /** Routines kept in config.json before they moved to the server: given to this project once, then dropped here. */
  async handOverLocal() {
    const local = this.ctx.config.values.routines;
    if (!local?.length) return;
    await sendToApi(this.ctx, 'POST', 'routines/import', { routines: local });
    delete this.ctx.config.values.routines;
    this.ctx.config.save();
  }

  /** Runs the server shows running on this browser that this app is not running: the app quit mid-run, so they were interrupted. */
  async markInterrupted() {
    const mine = (r) =>
      (r.runs || []).filter((run) => run.status === 'running' && run.by === this.ctx.socket.browserId);
    const orphans = this.list.flatMap((r) => mine(r).map((run) => [r.id, run.id]));
    const stale = orphans.filter(([, runId]) => runId !== this.current?.runId);
    for (const [id, runId] of stale) await this.end(id, runId, { status: 'interrupted' });
    if (stale.length) this.list = (await getFromApi(this.ctx, 'routines')).routines || [];
  }

  /** Sends one change to the server and re-reads the list; answers the snapshot, or `{ error }` saying why not. */
  async change(method, route, body) {
    if (!canCallServer(this.ctx)) return { error: 'Connect to Oya to change routines.' };
    try {
      await sendToApi(this.ctx, method, route, body);
    } catch (e) {
      return { error: e.message };
    }
    return this.refresh();
  }

  /** Adds a routine, or changes the one with its id (its history is kept). */
  save(input) {
    const { id, name, prompt, schedule, enabled } = input || {};
    const body = { name, prompt, schedule, enabled };
    return id
      ? this.change('PATCH', `routines/${encodeURIComponent(id)}`, body)
      : this.change('POST', 'routines', body);
  }

  /** Deletes a routine and its history. */
  remove(id) {
    return this.change('DELETE', `routines/${encodeURIComponent(id)}`);
  }

  /** Turns a routine's schedule on or off; a run in progress is not stopped (that is Stop). */
  setEnabled(id, enabled) {
    return this.change('PATCH', `routines/${encodeURIComponent(id)}`, { enabled: !!enabled });
  }

  /** Clears a routine's finished runs. */
  clearHistory(id) {
    return this.change('DELETE', `routines/${encodeURIComponent(id)}/runs`);
  }

  /** Stops this app's run of routine `id`, and only that; answers whether one was stopped. */
  stop(id) {
    if (this.current?.routineId !== id) return false;
    this.ctx.chatAbort?.abort();
    return true;
  }

  /** Re-reads the routines, then runs the first due one when this app is free. */
  async tick(now = Date.now()) {
    if (!canCallServer(this.ctx)) return;
    await this.refresh();
    const due = this.list.find((r) => isDue(r, now));
    if (due && !busyReason(this.ctx, this.current)) await this.perform(due, await this.claim(due).catch(() => null));
  }

  /** Runs one routine now: answers at once with the snapshot, or `{ error }` saying why it cannot run. */
  async runNow(id) {
    const reason = busyReason(this.ctx, this.current);
    const routine = this.list.find((r) => r.id === id);
    if (reason || !routine) return { error: reason || 'That routine is gone.' };
    const runId = await this.claim(routine).catch((e) => ({ error: e.message }));
    if (runId.error) return (await this.refresh(), runId);
    void this.perform(routine, runId);
    return this.snapshot();
  }

  /** Claims the routine's next run from the server for this browser; answers the run id, or throws (another desktop has it). */
  async claim(routine) {
    const runId = crypto.randomUUID();
    const body = { lastRunAt: routine.lastRunAt ?? null, runId, browserId: this.ctx.socket.browserId };
    await sendToApi(this.ctx, 'POST', `routines/${encodeURIComponent(routine.id)}/claim`, body);
    return runId;
  }

  /** Asks the agent the routine's prompt as the claimed run `runId`, then records how it ended. */
  async perform(routine, runId) {
    if (!runId) return this.refresh();
    this.current = { routineId: routine.id, runId };
    await this.refresh();
    const ask = [{ role: 'user', content: routine.prompt }];
    const answer = await this.run(this.ctx, null, ask).catch((e) => ({ error: e.message }));
    this.current = null;
    await this.end(routine.id, runId, endingOf(answer));
    await this.refresh();
  }

  /** Records a run's ending on the server; one that cannot be sent is left to the server's lease. */
  end(id, runId, ending) {
    const route = `routines/${encodeURIComponent(id)}/runs/${encodeURIComponent(runId)}`;
    return sendToApi(this.ctx, 'PATCH', route, ending).catch(() => {});
  }
}

module.exports = { Routines, isDue, nextRunAt };
