/**
 * The first-run welcome screen: one panel that switches between three views:
 * start (one-click "Sign in with Oya", which the dashboard answers with a
 * pairing link), waiting (for that sign-in to finish) and manual (server
 * address, API key and browser name, for self-hosters).
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { RendererServices } from '../../../app/services.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import { SERVER_URL, TEXT } from '../model/constants.ts';
import { messageOf, shape, type Config } from '../model/models.ts';

/** The panel's views. */
export type SetupView = 'start' | 'waiting' | 'manual';

/** The manual form's fields. */
export type SetupField = 'server' | 'apiKey' | 'name';

/** What the welcome screen shows. */
export interface SetupState extends Record<SetupField, string> {
  /** The view in the panel. */
  view: SetupView;
  /** The error under the form ('' for none). */
  error: string;
  /** Connect is waiting for the server. */
  busy: boolean;
}

/** The parts of the bridge the welcome screen uses. */
export type SetupBridge = Pick<
  OyaBrowser,
  'getConfig' | 'saveConfig' | 'openConsole' | 'enterBrowsing' | 'onWsStatus' | 'onModeChanged'
>;

/** Why the entered settings cannot be used, or '' when they can. */
export function setupProblem(server: string, key: string): string {
  if (!SERVER_URL.test(server)) return TEXT.badServer;
  return key ? '' : TEXT.noKey;
}

/** What the welcome screen needs from its neighbours. */
export interface SetupDeps extends Pick<RendererServices, 'shell'> {
  /** The main process. */
  bridge: SetupBridge;
}

/** The welcome screen. */
export class SetupViewModel extends ViewModel<SetupState> {
  /** The main process. */
  private readonly bridge: SetupBridge;
  /** Whether the browser is connected. */
  private readonly shell: RendererServices['shell'];
  /** Reports a failed connection after the wait. */
  private timer: ReturnType<typeof setTimeout> | undefined;

  /** On the start view, filled from the saved settings, following the connection and the mode. */
  constructor(deps: SetupDeps) {
    super({ view: 'start', server: '', apiKey: '', name: '', error: '', busy: false });
    this.bridge = deps.bridge;
    this.shell = deps.shell;
    this.own(() => clearTimeout(this.timer));
    this.own(this.bridge.onWsStatus((s) => s.connected && this.set({ busy: false, error: '' })));
    this.own(this.bridge.onModeChanged((mode) => mode !== 'browsing' && this.loggedOut()));
    void this.load();
  }

  /** Fills the form from the saved settings. */
  async load(): Promise<void> {
    const config = shape<Config>(await this.bridge.getConfig().catch(() => null));
    this.set({ server: config.serverUrl ?? '', apiKey: config.apiKey ?? '', name: config.browserName ?? '' });
  }

  /** Shows one of the panel's views, clearing the error. */
  show(view: SetupView): void {
    this.set({ view, error: '' });
  }

  /** A field was typed in. */
  edit(field: SetupField, value: string): void {
    this.set({ [field]: value });
  }

  /** Shows `error` under the form (the reconnect dialog uses it when the settings cannot be read). */
  showError(error: string): void {
    this.set({ error });
  }

  /** Saves the settings; the connection status switches modes, or the wait reports failure. */
  async connect(): Promise<void> {
    const [server, key, name] = [this.state.server, this.state.apiKey, this.state.name].map((v) => v.trim());
    const error = setupProblem(server, key);
    this.set({ error });
    if (error) return;
    this.set({ busy: true });
    await this.save({ serverUrl: server, apiKey: key, browserName: name || undefined });
  }

  /** Saves `settings`, then waits for the connection; or says why they could not be saved. */
  private async save(settings: Partial<Config>): Promise<void> {
    try {
      await this.bridge.saveConfig(settings);
    } catch (error) {
      return this.set({ error: messageOf(error, TEXT.saveFailed), busy: false });
    }
    this.wait();
  }

  /** Opens the dashboard, which sends back a pairing link once the person is signed in. */
  async signIn(): Promise<void> {
    this.set({ error: '' });
    const url = await this.bridge.openConsole(this.state.server);
    if (url) this.show('waiting');
    else this.set({ error: TEXT.badConsole });
  }

  /** "Open it": the dashboard of the server in the form. */
  openConsole(): void {
    void this.bridge.openConsole(this.state.server);
  }

  /** "Just browse for now". */
  skip(): void {
    void this.bridge.enterBrowsing();
  }

  /** Waits for the connection to report back. */
  private wait(): void {
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.timedOut(), C.CONNECT_TIMEOUT_MS);
  }

  /** No connection yet after the wait. */
  private timedOut(): void {
    this.set({ busy: false });
    if (!this.shell.state.connected) this.set({ error: TEXT.setupTimedOut });
  }

  /** Logged out: back to the start, with nothing of the old key left in the form. */
  private loggedOut(): void {
    this.set({ apiKey: '', view: 'start', error: '' });
  }
}
