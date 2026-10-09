/** Window-local services share the application session but never selection, overlays or page ownership. */
import type { AppServices } from '../app/services.ts';
import { ShellWindow } from '../shell/window.ts';
import { Shortcuts } from '../shell/shortcuts.ts';
import { ControlShield } from '../shell/control-shield.ts';
import { PanelLayout } from '../shell/layout.ts';
import { Overlays } from '../shell/overlays.ts';
import { TabManager } from '../tabs/tabs.ts';

/** Stateful collaborators belonging to a single native window. */
const LOCAL_SERVICES = new Set(['shell', 'shortcuts', 'shield', 'layout', 'overlays', 'tabs']);

/** Keeps assignments to shared task/session state on the application, not on a window shadow copy. */
export class WindowContext {
  /** The dependency scope held by all of this window's collaborators. */
  readonly services: AppServices;
  /** Construct each local service once; everything else resolves against the shared root. */
  constructor(root: AppServices) {
    const local = {} as AppServices;
    this.services = new Proxy(local, this.routing(root));
    Object.assign(local, this.locals(this.services));
  }
  /** Reads local collaborators and forwards all shared values, including writes, to the root. */
  private routing(root: AppServices): ProxyHandler<AppServices> {
    return {
      get: (local, key) => Reflect.get(LOCAL_SERVICES.has(String(key)) ? local : root, key),
      set: (local, key, value) => Reflect.set(LOCAL_SERVICES.has(String(key)) ? local : root, key, value),
    };
  }
  /** A complete local dependency graph; collaborators defer cross-service calls until startup. */
  private locals(scope: AppServices) {
    return {
      shell: new ShellWindow(scope),
      shortcuts: new Shortcuts(scope),
      shield: new ControlShield(scope),
      layout: new PanelLayout(scope),
      overlays: new Overlays(scope),
      tabs: new TabManager(scope),
    };
  }
}
