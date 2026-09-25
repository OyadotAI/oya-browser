/**
 * IPC: the Routines pane. Every change goes to the project's routines on the
 * server and answers the new list, or `{ error }` saying why not.
 */

/** Channel → handler. */
const ROUTINE_HANDLERS = {
  // Reads the server afresh, so the pane opens on what the project has now.
  'list-routines': (ctx) => ctx.routines.refresh(),
  'save-routine': (ctx, _e, routine) => ctx.routines.save(routine),
  'delete-routine': (ctx, _e, id) => ctx.routines.remove(id),
  'set-routine-enabled': (ctx, _e, id, enabled) => ctx.routines.setEnabled(id, enabled),
  'clear-routine-history': (ctx, _e, id) => ctx.routines.clearHistory(id),
  'stop-routine': (ctx, _e, id) => ctx.routines.stop(id),
  // Answers once the run is claimed: it can take minutes, and the pane follows it through routines-changed.
  'run-routine-now': (ctx, _e, id) => ctx.routines.runNow(id),
};

module.exports = { ROUTINE_HANDLERS };
