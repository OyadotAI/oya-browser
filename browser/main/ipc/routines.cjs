/** IPC: the Routines pane's list, edits and "Run now". */

/** Channel → handler. */
const ROUTINE_HANDLERS = {
  'list-routines': (ctx) => ctx.routines.snapshot(),
  'save-routine': (ctx, _e, routine) => ctx.routines.save(routine),
  'delete-routine': (ctx, _e, id) => ctx.routines.remove(id),
  // Answers at once: a run can take minutes, and the pane follows it through routines-changed.
  'run-routine-now': (ctx, _e, id) => {
    void ctx.routines.runNow(id);
    return ctx.routines.snapshot();
  },
};

module.exports = { ROUTINE_HANDLERS };
