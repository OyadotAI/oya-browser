/**
 * The Routines pane: the project's routines, the prompts its Oya apps run on a
 * schedule. They live on the server, so every desktop on the project shows the
 * same list and history. Each card says, in one line, what the routine is doing
 * (next run, running here or on another browser, off, or waiting and why), has
 * a switch that turns its schedule on or off, one action (Run now, or Stop for
 * a run on this browser), its history, and a menu for Edit, Clear history and
 * Delete. The main process keeps the list in step with the server
 * (main/routines.cjs) and pushes every change here; times redraw on a clock.
 */
/* global oyaBrowser, Dom, RendererConstants, RoutineRuns, RoutineEditor, ShellIcons */
/* exported Routines */

/** The words the pane shows. */
const ROUTINES_TEXT = {
  empty: 'No routines yet',
  emptyHint: 'Run a prompt every hour, or every day at a set time. Routines belong to this project.',
  offline: 'Offline. Connect to Oya to see and run this project’s routines.',
  loadError: (why) => `Could not reach this project’s routines: ${why}`,
  off: 'Off',
  dueNow: 'Due now',
  waiting: (why) => `Waiting: ${why}`,
  next: (when) => `Next ${when}`,
  runningHere: (elapsed) => `Running on this browser · ${elapsed}`,
  runningElsewhere: 'Running on another Oya browser',
  runNow: 'Run now',
  stop: 'Stop',
  history: (n) => (n ? `History · ${n}` : 'History'),
  more: (name) => `More for ${name}`,
  toggle: (name) => `Run “${name}” on its schedule`,
  edit: 'Edit',
  clear: 'Clear history',
  remove: 'Delete',
  confirm: (name) => `Delete “${name}” and its history?`,
  cancel: 'Cancel',
  every: (n, unit) => `Every ${n === 1 ? unit.replace(/s$/, '') : `${n} ${unit}`}`,
  daily: (at) => `Daily at ${at}`,
};

/** Schedule kind → how a card names it. */
const SCHEDULE_LABEL = {
  every: (s) => ROUTINES_TEXT.every(s.n, s.unit),
  daily: (s) => ROUTINES_TEXT.daily(s.at),
};

/** A routine's phase → its one-line status. */
const PHASE_LINE = {
  here: (r) => ROUTINES_TEXT.runningHere(RoutineRuns.duration({ ...r.runs[0], finishedAt: Date.now() })),
  elsewhere: () => ROUTINES_TEXT.runningElsewhere,
  off: (r) => `${Routines.scheduleOf(r)} · ${ROUTINES_TEXT.off}`,
  due: (r) =>
    `${Routines.scheduleOf(r)} · ${Routines.state.busy ? ROUTINES_TEXT.waiting(Routines.state.busy) : ROUTINES_TEXT.dueNow}`,
  scheduled: (r) => `${Routines.scheduleOf(r)} · ${ROUTINES_TEXT.next(RoutineRuns.when(r.nextRunAt))}`,
};

/** A small button. */
function routineButton(label, cls, onClick) {
  const button = Dom.node('button', label, cls);
  button.type = 'button';
  button.addEventListener('click', onClick);
  return button;
}

/** The Routines pane. */
const Routines = {
  /** What the main process last sent: the routines, the one running here, whether online, and why it is busy. */
  state: { routines: [], running: null, online: true, busy: '', error: '' },
  /** Ids of the routines whose history is showing. */
  expanded: new Set(),
  /** The routine whose menu is open, or null. */
  menu: null,
  /** The routine asking to confirm its deletion, or null. */
  confirming: null,
  /** Notes under routines (why Run now could not start), by id. */
  notes: new Map(),

  /** Loads the list from the server, through the main process. */
  async load() {
    Routines.apply(await oyaBrowser.listRoutines());
  },

  /** Shows a new list, or a refusal as a note under routine `id`. */
  apply(result, id) {
    if (result?.error) return Routines.note(id, result.error);
    Routines.show(result);
  },

  /** Shows a snapshot from the main process. */
  show(state) {
    if (state?.routines) Routines.state = state;
    const { online, error } = Routines.state;
    const banner = Dom.byId('routines-banner');
    banner.textContent = !online ? ROUTINES_TEXT.offline : error ? ROUTINES_TEXT.loadError(error) : '';
    banner.hidden = !banner.textContent;
    Routines.draw();
  },

  /** Redraws the cards, and keeps their times ticking while the pane shows. */
  draw() {
    const rows = Routines.state.routines.map(Routines.card);
    Dom.byId('routines-list').replaceChildren(...(rows.length ? rows : [Routines.empty()]));
    if (!Routines.clock && Routines.visible())
      Routines.clock = setInterval(Routines.tick, RendererConstants.ROUTINES_CLOCK_MS);
  },

  /** The clock that redraws times ("Next 10:02", a run's elapsed time), only while the pane shows. */
  clock: null,

  /** Whether the Routines pane is the one showing. */
  visible() {
    return Dom.byId('pane-routines').classList.contains('active');
  },

  /** What an empty project shows. */
  empty() {
    const li = Dom.node('li', null, 'routines-empty');
    li.append(Dom.node('strong', ROUTINES_TEXT.empty), Dom.node('span', ROUTINES_TEXT.emptyHint));
    return li;
  },

  /** A note under a routine for a few seconds (a refusal without a routine goes to the banner). */
  note(id, message) {
    if (!id) return Routines.show({ ...Routines.state, error: message });
    Routines.notes.set(id, message);
    Routines.draw();
    setTimeout(() => Routines.notes.delete(id) && Routines.draw(), RendererConstants.ROUTINE_NOTE_MS);
  },

  /** How the routine's schedule reads. */
  scheduleOf(routine) {
    const kind = routine.schedule?.kind;
    return Object.hasOwn(SCHEDULE_LABEL, kind) ? SCHEDULE_LABEL[kind](routine.schedule) : '';
  },

  /** What the routine is doing: running here or elsewhere, off, due, or scheduled. */
  phase(routine) {
    if (routine.id === Routines.state.running) return 'here';
    if (routine.runs?.[0]?.status === 'running') return 'elsewhere';
    if (!routine.enabled || !routine.nextRunAt) return 'off';
    return routine.nextRunAt <= Date.now() ? 'due' : 'scheduled';
  },

  /** One routine's card. */
  card(routine) {
    const phase = Routines.phase(routine);
    const li = Dom.node('li', null, `routine-card ${phase}`);
    li.dataset.id = routine.id;
    li.append(Routines.top(routine), Dom.node('div', PHASE_LINE[phase](routine), 'routine-status'));
    li.append(Routines.foot(routine, phase), ...Routines.extras(routine));
    return li;
  },

  /** The switch, the name, and how the last run went. */
  top(routine) {
    const top = Dom.node('div', null, 'routine-top');
    top.append(Routines.switch(routine), Dom.node('span', routine.name, 'routine-name'), RoutineRuns.last(routine));
    return top;
  },

  /** The on/off switch for the routine's schedule. */
  switch(routine) {
    const toggle = routineButton('', 'switch', () => Routines.setEnabled(routine));
    toggle.setAttribute('role', 'switch');
    toggle.setAttribute('aria-checked', String(!!routine.enabled));
    toggle.setAttribute('aria-label', ROUTINES_TEXT.toggle(routine.name));
    return toggle;
  },

  /** The main action (none while another browser runs it: the status line says so), the history toggle, and the menu. */
  foot(routine, phase) {
    const foot = Dom.node('div', null, 'routine-foot');
    if (phase !== 'elsewhere') foot.append(Routines.action(routine, phase));
    foot.append(Routines.historyButton(routine), Routines.moreButton(routine));
    if (Routines.menu === routine.id) foot.append(Routines.menuFor(routine));
    return foot;
  },

  /** Stop for a run here; otherwise Run now, disabled with the reason when this app cannot run it now. */
  action(routine, phase) {
    if (phase === 'here') return routineButton(ROUTINES_TEXT.stop, 'routine-action stop', () => Routines.stop(routine));
    const button = routineButton(ROUTINES_TEXT.runNow, 'routine-action', () => Routines.runNow(routine));
    Object.assign(button, { disabled: !!Routines.state.busy, title: Routines.state.busy || '' });
    return button;
  },

  /** "History · n", opening the runs. */
  historyButton(routine) {
    const label = ROUTINES_TEXT.history(routine.runs?.length || 0);
    const button = routineButton(label, 'routine-history-toggle', () => Routines.toggleHistory(routine));
    button.setAttribute('aria-expanded', String(Routines.expanded.has(routine.id)));
    const chevron = Dom.node('span', null, 'routine-chevron');
    chevron.innerHTML = ShellIcons.icon('chevron'); // the app's own icon
    button.append(chevron);
    return button;
  },

  /** The ⋯ button that opens Edit, Clear history and Delete. */
  moreButton(routine) {
    const button = routineButton('', 'routine-more', () => Routines.toggleMenu(routine));
    button.innerHTML = ShellIcons.icon('more'); // the app's own icon
    button.setAttribute('aria-label', ROUTINES_TEXT.more(routine.name));
    button.setAttribute('aria-haspopup', 'menu');
    button.setAttribute('aria-expanded', String(Routines.menu === routine.id));
    return button;
  },

  /** What sits under the card's footer: a note, the delete confirmation, and its history. */
  extras(routine) {
    const extras = [];
    if (Routines.notes.has(routine.id)) extras.push(Dom.node('p', Routines.notes.get(routine.id), 'routine-note'));
    if (Routines.confirming === routine.id) extras.push(Routines.confirmFor(routine));
    if (Routines.expanded.has(routine.id)) extras.push(RoutineRuns.list(routine.runs, Routines.state.browserId));
    return extras;
  },

  /** The menu: Edit, Clear history (when there is some) and Delete. */
  menuFor(routine) {
    const menu = Dom.node('div', null, 'routine-menu');
    menu.setAttribute('role', 'menu');
    menu.append(...Routines.menuItems(routine).map(([label, onClick]) => Routines.menuItem(label, onClick)));
    return menu;
  },

  /** The menu's items as [label, what choosing it does]; Clear history only when there is some. */
  menuItems(routine) {
    const finished = (routine.runs || []).some((run) => run.status !== 'running');
    const clear = finished ? [[ROUTINES_TEXT.clear, () => Routines.clearHistory(routine)]] : [];
    return [
      [ROUTINES_TEXT.edit, () => Routines.edit(routine)],
      ...clear,
      [ROUTINES_TEXT.remove, () => Routines.askDelete(routine)],
    ];
  },

  /** One menu item; choosing it closes the menu. */
  menuItem(label, onClick) {
    const item = routineButton(label, label === ROUTINES_TEXT.remove ? 'danger' : '', () => {
      Routines.menu = null;
      onClick();
    });
    item.setAttribute('role', 'menuitem');
    return item;
  },

  /** "Delete “name” and its history?" with Cancel and Delete. */
  confirmFor(routine) {
    const box = Dom.node('div', null, 'routine-confirm');
    box.append(Dom.node('span', ROUTINES_TEXT.confirm(routine.name)));
    box.append(routineButton(ROUTINES_TEXT.cancel, 'text-button', () => Routines.askDelete(null)));
    box.append(routineButton(ROUTINES_TEXT.remove, 'routine-action stop', () => Routines.remove(routine)));
    return box;
  },

  /** Opens or closes a routine's menu. */
  toggleMenu(routine) {
    Routines.menu = Routines.menu === routine.id ? null : routine.id;
    Routines.draw();
  },

  /** A press outside an open menu, or Escape, closes it. */
  closeMenu(e) {
    if (!Routines.menu || (e.type === 'keydown' && e.key !== 'Escape')) return;
    if (e.type === 'mousedown' && e.target.closest?.('.routine-menu, .routine-more')) return;
    Routines.menu = null;
    Routines.draw();
  },

  /** Shows or hides a routine's history, opening its latest run. */
  toggleHistory(routine) {
    if (Routines.expanded.delete(routine.id)) return Routines.draw();
    Routines.expanded.add(routine.id);
    if (routine.runs?.[0]) RoutineRuns.open.add(routine.runs[0].id);
    Routines.draw();
  },

  /** Asks to confirm deleting `routine`, or cancels with null. */
  askDelete(routine) {
    Routines.confirming = routine?.id || null;
    Routines.draw();
  },

  /** Opens the editor on a routine. */
  edit(routine) {
    RoutineEditor.open(routine);
    Routines.draw();
  },

  /** Turns the routine's schedule on or off. */
  async setEnabled(routine) {
    Routines.apply(await oyaBrowser.setRoutineEnabled(routine.id, !routine.enabled), routine.id);
  },

  /** Runs the routine now; why it could not start shows under it. */
  async runNow(routine) {
    Routines.apply(await oyaBrowser.runRoutineNow(routine.id), routine.id);
  },

  /** Stops this browser's run of the routine. */
  async stop(routine) {
    await oyaBrowser.stopRoutine(routine.id);
  },

  /** Clears the routine's finished runs. */
  async clearHistory(routine) {
    Routines.apply(await oyaBrowser.clearRoutineHistory(routine.id), routine.id);
  },

  /** Deletes the routine; the editor stops editing it. */
  async remove(routine) {
    Routines.confirming = null;
    if (RoutineEditor.editing?.id === routine.id) RoutineEditor.close();
    Routines.apply(await oyaBrowser.deleteRoutine(routine.id), routine.id);
  },

  /** Redraws the times; once the pane is hidden, the clock stops until it shows again. */
  tick() {
    if (Routines.visible()) return Routines.draw();
    clearInterval(Routines.clock);
    Routines.clock = null;
  },
};

RoutineEditor.saved = Routines.show;
Dom.byId('routine-new').addEventListener('click', () => RoutineEditor.open(null));
document.addEventListener('mousedown', Routines.closeMenu);
document.addEventListener('keydown', Routines.closeMenu);
oyaBrowser.onRoutinesChanged(Routines.show);
// Opening the tab redraws (fresh times) and starts the clock; the tab shows its pane first.
document.querySelector('[data-pane="routines"]')?.addEventListener('click', () => setTimeout(Routines.draw));
Routines.load();
