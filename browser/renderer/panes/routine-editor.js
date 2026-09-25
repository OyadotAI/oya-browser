/**
 * The Routines pane's editor: a card at the top of the pane for a new routine
 * or the one being edited, with its name, prompt and schedule ("every N minutes
 * or hours", or "daily at HH:MM"). Saving goes to the project on the server; a
 * refusal (a name too long, a time not written HH:MM) shows in the card, which
 * keeps what was typed.
 */
/* global oyaBrowser, Dom */
/* exported RoutineEditor */

/** The words the editor shows. */
const EDITOR_TEXT = {
  newTitle: 'New routine',
  editTitle: 'Edit routine',
  create: 'Save routine',
  update: 'Save changes',
};

/** A new routine's schedule until one is picked. */
const DEFAULT_SCHEDULE = { kind: 'every', n: 1, unit: 'hours' };

/** The routine editor. */
const RoutineEditor = {
  /** The routine being edited, or null for a new one. */
  editing: null,
  /** The schedule kind picked: every or daily. */
  kind: 'every',
  /** Called with the new list once a save went through (set by the pane). */
  saved: () => {},

  /** Opens the editor on `routine`, or empty for a new one. */
  open(routine) {
    RoutineEditor.editing = routine || null;
    Dom.byId('routine-form-title').textContent = routine ? EDITOR_TEXT.editTitle : EDITOR_TEXT.newTitle;
    Dom.byId('routine-save').textContent = routine ? EDITOR_TEXT.update : EDITOR_TEXT.create;
    RoutineEditor.fill(routine);
    Dom.byId('routine-form').hidden = false;
    Dom.byId('routine-form').scrollIntoView?.({ block: 'nearest' });
    Dom.byId('routine-name').focus();
  },

  /** Puts `routine`'s settings (or a new routine's) into the fields. */
  fill(routine) {
    Dom.byId('routine-name').value = routine?.name || '';
    Dom.byId('routine-prompt').value = routine?.prompt || '';
    const schedule = routine?.schedule || DEFAULT_SCHEDULE;
    if (schedule.kind === 'every') Dom.byId('routine-n').value = String(schedule.n);
    if (schedule.kind === 'every') Dom.byId('routine-unit').value = schedule.unit;
    if (schedule.kind === 'daily') Dom.byId('routine-at').value = schedule.at;
    RoutineEditor.setKind(schedule.kind);
    RoutineEditor.error('');
  },

  /** Picks the schedule kind, showing its fields. */
  setKind(kind) {
    RoutineEditor.kind = kind;
    Dom.byId('routine-kind')
      .querySelectorAll('[data-kind]')
      .forEach((b) => b.setAttribute('aria-checked', String(b.dataset.kind === kind)));
    Dom.byId('routine-n').hidden = kind === 'daily';
    Dom.byId('routine-unit').hidden = kind === 'daily';
    Dom.byId('routine-at').hidden = kind !== 'daily';
  },

  /** A click on Every or Daily at. */
  kindClicked(e) {
    const button = e.target.closest('[data-kind]');
    if (button) RoutineEditor.setKind(button.dataset.kind);
  },

  /** The schedule the fields describe. */
  schedule() {
    if (RoutineEditor.kind === 'daily') return { kind: 'daily', at: Dom.byId('routine-at').value };
    return { kind: 'every', n: Number(Dom.byId('routine-n').value), unit: Dom.byId('routine-unit').value };
  },

  /** What the fields hold, as a routine to save; an edit keeps its id and whether it is on. */
  routine() {
    const { editing } = RoutineEditor;
    const text = { name: Dom.byId('routine-name').value, prompt: Dom.byId('routine-prompt').value };
    return { ...text, id: editing?.id, enabled: editing?.enabled ?? true, schedule: RoutineEditor.schedule() };
  },

  /** Saves to the project; a refusal stays in the card with what was typed. */
  async submit(e) {
    e.preventDefault();
    const result = await oyaBrowser.saveRoutine(RoutineEditor.routine()).catch((err) => ({ error: err.message }));
    if (result?.error) return RoutineEditor.error(result.error);
    RoutineEditor.close();
    RoutineEditor.saved(result);
  },

  /** Shows why the save was refused, or clears it. */
  error(message) {
    const note = Dom.byId('routine-error');
    note.textContent = String(message).replace(/^Error invoking remote method '[^']+': (Error: )?/, '');
    note.hidden = !message;
  },

  /** Closes the editor. */
  close() {
    RoutineEditor.editing = null;
    Dom.byId('routine-form').hidden = true;
  },
};

Dom.byId('routine-form').addEventListener('submit', RoutineEditor.submit);
Dom.byId('routine-cancel').addEventListener('click', RoutineEditor.close);
Dom.byId('routine-kind').addEventListener('click', RoutineEditor.kindClicked);
