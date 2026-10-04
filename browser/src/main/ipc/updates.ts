/** IPC: the toolbar's update button and the app's version. */
import type { AppServices } from '../app/services.ts';
import type { HandlersOf } from './handle.ts';

/** The services the update handlers use. */
type Deps = Pick<AppServices, 'updater'>;

/** The channels this group answers. */
type Channel = 'check-for-updates' | 'get-update-status' | 'install-update' | 'get-version';

/** The updater, as the toolbar asks of it. */
export class UpdateHandlers {
  /** Channel → handler. */
  readonly handlers: HandlersOf<Channel> = {
    'check-for-updates': () => this.deps.updater.checkNow(),
    'get-update-status': () => this.deps.updater.state,
    'install-update': () => this.deps.updater.install(),
    'get-version': () => this.deps.updater.version(),
  };
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` gives the updater. */
  constructor(deps: Deps) {
    this.deps = deps;
  }
}
