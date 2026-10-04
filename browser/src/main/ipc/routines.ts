/**
 * IPC: the Routines pane. Every change goes to the project's routines on the
 * server and answers the new list, or `{ error }` saying why not.
 */
import type { AppServices } from '../app/services.ts';
import type { HandlersOf } from './handle.ts';

/** The services the routine handlers use. */
type Deps = Pick<AppServices, 'routines'>;

/** The channels this group answers. */
type Channel =
  | 'list-routines'
  | 'save-routine'
  | 'delete-routine'
  | 'set-routine-enabled'
  | 'clear-routine-history'
  | 'stop-routine'
  | 'run-routine-now';

/** The Routines pane's calls, each sent to the scheduler. */
export class RoutineHandlers {
  /** Channel → handler. */
  readonly handlers: HandlersOf<Channel> = {
    // Reads the server afresh, so the pane opens on what the project has now.
    'list-routines': () => this.deps.routines.refresh(),
    'save-routine': (_e, routine) => this.deps.routines.save(routine),
    'delete-routine': (_e, id) => this.deps.routines.remove(id),
    'set-routine-enabled': (_e, id, enabled) => this.deps.routines.setEnabled(id, enabled),
    'clear-routine-history': (_e, id) => this.deps.routines.clearHistory(id),
    'stop-routine': (_e, id) => this.deps.routines.stop(id),
    // Answers once the run is claimed: it can take minutes, and the pane follows it through routines-changed.
    'run-routine-now': (_e, id) => this.deps.routines.runNow(id),
  };
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` gives the scheduler. */
  constructor(deps: Deps) {
    this.deps = deps;
  }
}
