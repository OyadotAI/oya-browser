/**
 * Appearance: system, light or dark, following the OS when set to system,
 * and the saved UI preferences (theme, platform) at start. A change applies
 * all at once: `switching` holds for two frames so controls do not fade from
 * one theme to the other (html[data-theme-switching]).
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { RendererServices } from '../../../app/services.ts';
import type { Payload } from '../../../../shared/ipc.ts';
import { THEMES, type AppliedTheme, type ThemePreference } from '../model/constants.ts';

/** The shell's appearance. */
export interface ThemeState {
  /** The chosen appearance. */
  theme: ThemePreference;
  /** Whether the OS is in dark mode. */
  systemDark: boolean;
  /** The platform the main process reported (html[data-platform]), '' before it has. */
  platform: string;
  /** A theme change is being applied: transitions are off. */
  switching: boolean;
}

/** What the theme uses. */
export interface ThemeDeps extends Pick<RendererServices, 'frames'> {
  /** The saved preferences, and the OS appearance. */
  bridge: Pick<OyaBrowser, 'getUiPreferences' | 'saveUiPreferences' | 'onShellAppearance'>;
  /** Whether the OS was dark at the first paint (html[data-theme] from core/first-paint.js, else the media query). */
  systemDark: boolean;
}

/** The theme drawn for `state`. */
export const appliedTheme = ({ theme, systemDark }: ThemeState): AppliedTheme =>
  theme === 'system' ? (systemDark ? 'dark' : 'light') : theme;

/** `value` if it is an appearance, else system. */
export const themeOf = (value: unknown): ThemePreference =>
  THEMES.includes(value as ThemePreference) ? (value as ThemePreference) : 'system';

/** The shell's theme. */
export class ThemeViewModel extends ViewModel<ThemeState> {
  /** What it uses. */
  private readonly deps: ThemeDeps;
  /** The frame that ends a switch, if one is pending. */
  private frame: number | null = null;

  /** Applies the first-paint appearance, then follows the OS and restores the saved preferences. */
  constructor(deps: ThemeDeps) {
    super({ theme: 'system', systemDark: deps.systemDark, platform: '', switching: false });
    this.deps = deps;
    this.own(() => this.frame !== null && deps.frames.cancel(this.frame));
    this.own(deps.bridge.onShellAppearance((dark) => this.apply({ systemDark: dark })));
    this.apply({});
    void deps.bridge.getUiPreferences().then((value) => this.restore(value ?? {}));
  }

  /** The person picked an appearance: it applies and is saved. */
  choose(theme: ThemePreference): void {
    this.apply({ theme });
    void this.deps.bridge.saveUiPreferences({ theme });
  }

  /** Shows `theme` without saving it (the Electron suites' screenshots in each theme). */
  show(theme: ThemePreference): void {
    this.apply({ theme });
  }

  /** Restores the saved preferences. */
  private restore(value: Payload): void {
    const systemDark = typeof value.systemDark === 'boolean' ? value.systemDark : this.state.systemDark;
    this.apply({ theme: themeOf(value.theme), systemDark, platform: String(value.platform ?? '') });
  }

  /** Changes the appearance, with transitions off until two frames have drawn it. */
  private apply(patch: Partial<ThemeState>): void {
    this.set({ ...patch, switching: true });
    if (this.frame !== null) this.deps.frames.cancel(this.frame);
    const { frames } = this.deps;
    this.frame = frames.request(() => (this.frame = frames.request(() => this.settle())));
  }

  /** The switch has drawn: transitions come back. */
  private settle(): void {
    this.frame = null;
    this.set({ switching: false });
  }
}
