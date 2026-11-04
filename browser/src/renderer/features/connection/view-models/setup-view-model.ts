/**
 * The first-run welcome screen: one panel that switches between its views:
 * accounts (sign back in to one this browser signed out of), start (one-click "Sign in with Oya", which the dashboard answers with a
 * pairing link), waiting (for that sign-in to finish) and manual (server
 * address, API key and browser name, for self-hosters).
 */
import type { ConnectionStatus, SavedAccountView } from '../../../../shared/ipc.ts';
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { RendererServices } from '../../../app/services.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import { SERVER_URL, TEXT } from '../model/constants.ts';
import { messageOf, shape, type Config } from '../model/models.ts';
import { OnboardingViewModel, type OnboardingBridge } from './onboarding-view-model.ts';

/** The panel's views. */
export type SetupView = 'accounts' | 'checklist' | 'waiting' | 'manual';

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
  /** Accounts this browser signed out of, newest first. */
  accounts: SavedAccountView[];
  /** The account being signed back in to ('' for none). */
  choosing: string;
}

/** The parts of the bridge the welcome screen uses. */
export type SetupBridge = OnboardingBridge &
  Pick<
    OyaBrowser,
    | 'makeDefaultBrowser'
    | 'getConfig'
    | 'saveConfig'
    | 'openConsole'
    | 'enterBrowsing'
    | 'onWsStatus'
    | 'onModeChanged'
    | 'savedAccounts'
    | 'useSavedAccount'
    | 'forgetSavedAccount'
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
  /** First-run choices share this form's connection status without duplicating its fields. */
  readonly onboarding: OnboardingViewModel;
  /** Explicit opt-in from the welcome screen; native UI handles confirmation and errors. */
  async makeDefaultBrowser(): Promise<void> {
    await this.bridge.makeDefaultBrowser();
  }

  /** The main process. */
  private readonly bridge: SetupBridge;
  /** Whether the browser is connected. */
  private readonly shell: RendererServices['shell'];
  /** Reports a failed connection after the wait. */
  private timer: ReturnType<typeof setTimeout> | undefined;

  /** On the start view, filled from the saved settings, following the connection and the mode. */
  constructor(deps: SetupDeps) {
    super({ view: 'checklist', server: '', apiKey: '', name: '', error: '', busy: false, accounts: [], choosing: '' });
    this.bridge = deps.bridge;
    this.shell = deps.shell;
    this.onboarding = new OnboardingViewModel(deps.bridge, () => this.show('checklist'));
    this.watch();
    void this.load();
  }

  /** One owned lifetime covers first-run progress, mode and socket subscriptions. */
  private watch(): void {
    this.own(() => this.onboarding.dispose());
    this.own(() => clearTimeout(this.timer));
    this.own(this.bridge.onWsStatus((s) => this.connectionStatus(s)));
    this.own(this.bridge.onModeChanged((mode) => mode !== 'browsing' && this.loggedOut()));
  }

  /** Terminal setup failures stop the spinner immediately and preserve the recovery instruction. */
  private connectionStatus(status: ConnectionStatus): void {
    if (!status.connected && !status.failure) return;
    clearTimeout(this.timer);
    this.set({ busy: false, choosing: '', error: status.failure || '' });
  }

  /** Fills the form from the saved settings. */
  async load(): Promise<void> {
    const config = shape<Config>(await this.bridge.getConfig().catch(() => null));
    this.set({ server: config.serverUrl ?? '', apiKey: config.apiKey ?? '', name: config.browserName ?? '' });
    await this.loadAccounts();
  }

  /** The remembered accounts; while there are any, choosing one is the first view. */
  private async loadAccounts(): Promise<void> {
    this.showAccounts(await this.bridge.savedAccounts().catch(() => []));
  }

  /** Lists `answer` when it is a list, choosing among them first while there are any. */
  private showAccounts(answer: unknown): void {
    const accounts = Array.isArray(answer) ? (answer as SavedAccountView[]) : [];
    this.set({ accounts, view: accounts.length ? 'accounts' : 'checklist' });
  }

  /** Signs back in to a remembered account, then waits for the connection like any sign-in. */
  async useAccount(id: string): Promise<void> {
    this.set({ busy: true, error: '', choosing: id });
    try {
      await this.bridge.useSavedAccount(id);
    } catch (error) {
      this.set({ error: messageOf(error, TEXT.saveFailed), busy: false, choosing: '' });
      return this.loadAccounts();
    }
    this.wait();
  }

  /** Removes a remembered account from this browser. */
  async forgetAccount(id: string): Promise<void> {
    this.showAccounts(await this.bridge.forgetSavedAccount(id).catch(() => this.state.accounts));
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
    if (this.state.busy) return;
    const [server, key, name] = [this.state.server, this.state.apiKey, this.state.name].map((v) => v.trim());
    const error = setupProblem(server, key);
    this.set({ error });
    if (error) return;
    if (!(await this.onboarding.begin())) return;
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

  /** Retries main-owned settings without replacing the paired workspace or its sealed key. */
  async reconnect(): Promise<void> {
    if (this.state.busy || !(await this.onboarding.begin())) return;
    this.set({ busy: true, error: '' });
    await this.save({});
  }

  /** Opens the dashboard, which sends back a pairing link once the person is signed in. */
  async signIn(): Promise<void> {
    const current = this.onboarding.guard();
    this.set({ error: '' });
    if (!(await this.onboarding.begin())) return;
    const url = await this.bridge.openConsole(this.state.server).catch(() => null);
    if (!current()) return;
    if (url) this.show('waiting');
    else this.set({ error: TEXT.badConsole });
  }

  /** "Open it": the dashboard of the server in the form. */
  openConsole(): void {
    void this.bridge.openConsole(this.state.server);
  }

  /** "Just browse for now". */
  skip(): void {
    this.onboarding.finish();
  }

  /** Waits for the connection to report back. */
  private wait(): void {
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.timedOut(), C.CONNECT_TIMEOUT_MS);
  }

  /** No connection yet after the wait. */
  private timedOut(): void {
    this.set({ busy: false, choosing: '' });
    if (!this.shell.state.connected) this.set({ error: TEXT.setupTimedOut });
  }

  /** Logged out: back to the start, with nothing of the old key left in the form, offering the remembered accounts. */
  private loggedOut(): void {
    this.onboarding.choose(null);
    this.set({ apiKey: '', view: 'checklist', error: '' });
    void this.loadAccounts();
  }
}
