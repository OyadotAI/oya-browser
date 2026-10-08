/** IPC: overlays over the page, the dev panel, and the shell's own preferences. */
import { DefaultBrowser } from '../app/default-browser.ts';
import type { AppServices } from '../app/services.ts';
import type { Payload } from '../../shared/ipc.ts';
import type { HandlersOf } from './handle.ts';
import { FORMATS, DEFAULT_FORMAT } from '../../page/render.ts';
import { THEMES } from './constants.ts';

/** The services the shell handlers use. */
type Deps = Pick<AppServices, 'overlays' | 'layout' | 'config' | 'shell' | 'electron'>;

/** The channels this group answers. */
type Channel =
  | 'make-default-browser'
  | 'backdrop-ready'
  | 'show-overlay'
  | 'hide-overlay'
  | 'toggle-dev-panel'
  | 'resize-dev-panel'
  | 'get-ui-preferences'
  | 'save-ui-preferences';

/** Each preference the shell may save, and the values it accepts. */
const PREFERENCES: Readonly<Record<string, readonly unknown[]>> = {
  theme: THEMES,
  pageFormat: FORMATS,
  importOffered: [true],
};

/** The shell's overlays, panel and preferences, as it asks for them. */
export class ShellPageHandlers {
  /** Channel → handler. */
  readonly handlers: HandlersOf<Channel> = {
    'make-default-browser': () => new DefaultBrowser(this.deps).request(),
    'backdrop-ready': (_e, token) => this.deps.overlays.backdropReady(token),
    'show-overlay': (_e, name) => this.deps.overlays.show(name),
    'hide-overlay': (_e, name) => this.deps.overlays.hide(name),
    'toggle-dev-panel': (_e, reducedMotion) => this.deps.layout.toggle(reducedMotion),
    'resize-dev-panel': (_e, width) => this.deps.layout.resize(width),
    'get-ui-preferences': () => this.preferences(),
    'save-ui-preferences': (_e, preferences) => this.savePreferences(preferences),
  };
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** The shell's appearance settings, the page formats and the platform. */
  private preferences(): Payload {
    return {
      theme: 'system',
      pageFormat: DEFAULT_FORMAT,
      pageFormats: FORMATS,
      ...this.deps.config.values.ui,
      platform: process.platform,
      systemDark: this.deps.electron.nativeTheme.shouldUseDarkColors,
    };
  }

  /** Saves the theme and page format the shell chose, ignoring anything else. */
  private savePreferences(preferences: Payload | null): boolean {
    if (!preferences || typeof preferences !== 'object') return false;
    const ui = { ...this.deps.config.values.ui };
    for (const [key, allowed] of Object.entries(PREFERENCES))
      if (allowed.includes(preferences[key])) ui[key] = preferences[key];
    this.deps.config.values.ui = ui;
    this.deps.config.save();
    this.deps.shell.window?.setBackgroundColor(this.deps.shell.background());
    return true;
  }
}
