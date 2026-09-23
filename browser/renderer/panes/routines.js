/**
 * The Routines pane: prompts the agent runs on a schedule while the app is
 * open. Each row shows how the last run went, when the next one is, the last
 * answer, and its buttons: Run now, History, Pause, Edit and Delete. History
 * opens the routine's past runs (routine-runs.js). The form below adds a
 * routine, or edits the one picked. The main process keeps and runs them
 * (main/routines.cjs) and pushes every change here.
 */
/* global oyaBrowser, Dom, RendererConstants, RoutineRuns */
/* exported Routines */

/** The words the pane shows. */
const ROUTINES_TEXT = {
  empty: 'No routines yet. Add one below to run a prompt on a schedule.',
  newTitle: 'New routine',
  editTitle: 'Edit routine',
  paused: 'Paused',
  dueNow: 'Due now',
  next: (when) => `Next ${when}`,
  history: (n) => (n ? `History (${n})` : 'History'),
  hideHistory: 'Hide history',
  runNow: 'Run now',
  stop: 'Stop',
  pause: 'Pause',
  resume: 'Resume',
  edit: 'Edit',
  remove: 'Delete',
  every: (n, unit) => `Every ${n} ${n === 1 ? unit.replace(/s$/, '') : unit}`,
  daily: (at) => `Daily at ${at}`,
};

/** Schedule kind → how a row names it. */
const SCHEDULE_LABEL = {
  every: (s) => ROUTINES_TEXT.every(s.n, s.unit),
  daily: (s) => ROUTINES_TEXT.daily(s.at),
};

/** The Routines pane. */
const Routines = {
  /** The routines as the main process last sent them. */
  list: [],
  /** The id of the routine running now, or null. */
  running: null,
  /** The id of the routine the form is editing, or null for a new one. */
  editing: null,
  /** Ids of the routines whose history is showing. */
  expanded: new Set(),

  /** Loads the list from the main process. */
  async load() {
    Routines.show(await oyaBrowser.listRoutines());
  },

  /** Shows a snapshot of the list and the one running. */
  show(state) {
    Routines.list = state?.routines || [];
    Routines.running = state?.running || null;
    const rows = Routines.list.map(Routines.row);
    const box = Dom.byId('routines-list');
    box.replaceChildren(...(rows.length ? rows : [Dom.node('li', ROUTINES_TEXT.empty, 'routines-empty')]));
  },

  /** One routine's row: name and last status, schedule and next run, last answer, buttons, and its history when open. */
  row(routine) {
    const li = Dom.node('li', null, 'routine-row');
    li.classList.toggle('paused', !routine.enabled);
    li.append(Routines.head(routine), Dom.node('div', Routines.when(routine), 'routine-when'));
    const last = routine.runs?.[0];
    if (last?.result && last.status !== 'running') li.append(Routines.result(last));
    li.append(Routines.buttons(routine));
    if (Routines.expanded.has(routine.id)) li.append(RoutineRuns.list(routine.runs));
    return li;
  },

  /** The name, with how its last run went. */
  head(routine) {
    const head = Dom.node('div', null, 'routine-head');
    head.append(Dom.node('span', routine.name, 'routine-name'), RoutineRuns.badge(routine.runs?.[0]?.status));
    return head;
  },

  /** The schedule, and when it runs next (or that it is paused). */
  when(routine) {
    const schedule = SCHEDULE_LABEL[routine.schedule.kind](routine.schedule);
    if (!routine.enabled) return `${schedule} · ${ROUTINES_TEXT.paused}`;
    if (routine.id === Routines.running || !routine.nextRunAt) return schedule;
    const next =
      routine.nextRunAt <= Date.now() ? ROUTINES_TEXT.dueNow : ROUTINES_TEXT.next(RoutineRuns.when(routine.nextRunAt));
    return `${schedule} · ${next}`;
  },

  /** The last run's answer, cut short, marked when it failed. */
  result(run) {
    const cut = RendererConstants.ROUTINE_RESULT_SHOWN;
    const p = Dom.node('p', run.result.length > cut ? run.result.slice(0, cut) + '…' : run.result, 'routine-result');
    p.classList.toggle('error', run.status === 'failed');
    return p;
  },

  /** Run now (or Stop while it runs), History, Pause or Resume, Edit and Delete. */
  buttons(routine) {
    const bar = Dom.node('div', null, 'routine-buttons');
    bar.append(...Routines.actions(routine).map(([label, onClick]) => Routines.button(label, onClick)));
    return bar;
  },

  /** A routine's buttons, as [label, what a click does]. */
  actions(routine) {
    const running = routine.id === Routines.running;
    return [
      [ROUTINES_TEXT[running ? 'stop' : 'runNow'], () => Routines.runOrStop(routine, running)],
      [Routines.historyLabel(routine), () => Routines.toggleHistory(routine)],
      [ROUTINES_TEXT[routine.enabled ? 'pause' : 'resume'], () => Routines.toggle(routine)],
      [ROUTINES_TEXT.edit, () => Routines.edit(routine)],
      [ROUTINES_TEXT.remove, () => Routines.remove(routine)],
    ];
  },

  /** "History (n)", or "Hide history" while it shows. */
  historyLabel(routine) {
    if (Routines.expanded.has(routine.id)) return ROUTINES_TEXT.hideHistory;
    return ROUTINES_TEXT.history(routine.runs?.length || 0);
  },

  /** Shows or hides a routine's history. */
  toggleHistory(routine) {
    if (Routines.expanded.has(routine.id)) Routines.expanded.delete(routine.id);
    else Routines.expanded.add(routine.id);
    Routines.show({ routines: Routines.list, running: Routines.running });
  },

  /** A small text button. */
  button(label, onClick) {
    const button = Dom.node('button', label, 'text-button');
    button.type = 'button';
    button.addEventListener('click', onClick);
    return button;
  },

  /** Runs the routine now, or stops it when it is the one running. */
  async runOrStop(routine, running) {
    if (running) return oyaBrowser.stopChat();
    Routines.show(await oyaBrowser.runRoutineNow(routine.id));
  },

  /** Pauses a routine, or resumes a paused one. */
  async toggle(routine) {
    Routines.show(await oyaBrowser.saveRoutine({ ...routine, enabled: !routine.enabled }));
  },

  /** Deletes a routine; the form stops editing it. */
  async remove(routine) {
    if (Routines.editing === routine.id) Routines.reset();
    Routines.show(await oyaBrowser.deleteRoutine(routine.id));
  },

  /** Fills the form with a routine to edit. */
  edit(routine) {
    Routines.editing = routine.id;
    Dom.byId('routine-name').value = routine.name;
    Dom.byId('routine-prompt').value = routine.prompt;
    Routines.fillSchedule(routine.schedule);
    Dom.byId('routine-form-title').textContent = ROUTINES_TEXT.editTitle;
    Dom.byId('routine-cancel').hidden = false;
    Dom.byId('routine-name').focus();
  },

  /** Sets the schedule fields from a saved schedule. */
  fillSchedule(schedule) {
    Dom.byId('routine-kind').value = schedule.kind;
    if (schedule.kind === 'every') Dom.byId('routine-n').value = String(schedule.n);
    if (schedule.kind === 'every') Dom.byId('routine-unit').value = schedule.unit;
    if (schedule.kind === 'daily') Dom.byId('routine-at').value = schedule.at;
    Routines.kindChanged();
  },

  /** Shows the fields of the chosen schedule kind. */
  kindChanged() {
    const daily = Dom.byId('routine-kind').value === 'daily';
    Dom.byId('routine-n').hidden = daily;
    Dom.byId('routine-unit').hidden = daily;
    Dom.byId('routine-at').hidden = !daily;
  },

  /** The schedule the form describes. */
  schedule() {
    if (Dom.byId('routine-kind').value === 'daily') return { kind: 'daily', at: Dom.byId('routine-at').value };
    return { kind: 'every', n: Number(Dom.byId('routine-n').value), unit: Dom.byId('routine-unit').value };
  },

  /** What the form holds, as a routine to save. */
  fromForm() {
    const old = Routines.list.find((r) => r.id === Routines.editing);
    const routine = { name: Dom.byId('routine-name').value, prompt: Dom.byId('routine-prompt').value };
    return {
      ...routine,
      id: Routines.editing || undefined,
      enabled: old?.enabled ?? true,
      schedule: Routines.schedule(),
    };
  },

  /** Saves the form; a refusal is shown under it and the form keeps what was typed. */
  async submit(e) {
    e.preventDefault();
    try {
      Routines.show(await oyaBrowser.saveRoutine(Routines.fromForm()));
      Routines.reset();
    } catch (err) {
      Routines.refused(err);
    }
  },

  /** Shows why the main process refused the form, without Electron's wrapping. */
  refused(err) {
    const error = Dom.byId('routine-error');
    error.textContent = String(err.message || err).replace(/^Error invoking remote method '[^']+': (Error: )?/, '');
    error.hidden = false;
  },

  /** Empties the form back to a new routine. */
  reset() {
    Routines.editing = null;
    Dom.byId('routine-name').value = '';
    Dom.byId('routine-prompt').value = '';
    Dom.byId('routine-error').hidden = true;
    Dom.byId('routine-form-title').textContent = ROUTINES_TEXT.newTitle;
    Dom.byId('routine-cancel').hidden = true;
  },
};

Dom.byId('routine-form').addEventListener('submit', Routines.submit);
Dom.byId('routine-cancel').addEventListener('click', Routines.reset);
Dom.byId('routine-kind').addEventListener('change', Routines.kindChanged);
oyaBrowser.onRoutinesChanged(Routines.show);
Routines.load();
