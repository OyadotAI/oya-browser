/**
 * The connection settings dialog, opened from the account page's Advanced
 * while browsing: edit the server, key and name, then save and wait for the
 * connection to report back. A connection closes it.
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { RendererServices } from '../../../app/services.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import { OVERLAYS, SERVER_URL, TEXT } from '../model/constants.ts';
import { messageOf, shape, type Config } from '../model/models.ts';
import type { ShellDialogViewModel } from './shell-dialog-view-model.ts';
import type { SetupViewModel } from './setup-view-model.ts';

/** The form's fields. */
export type ReconnectField = 'server' | 'apiKey' | 'name';

/** What the dialog shows. */
export interface ReconnectState extends Record<ReconnectField, string> {
  /** The dialog is open. */
  open: boolean;
  /** The error under the form ('' for none). */
  error: string;
  /** Saving or waiting for the connection: the button is disabled. */
  saving: boolean;
  /** The save button's words. */
  button: string;
}

/** The parts of the bridge the dialog uses. */
export type ReconnectBridge = Pick<
  OyaBrowser,
  'showOverlay' | 'hideOverlay' | 'getConfig' | 'saveConfig' | 'onWsStatus'
>;

/** What the dialog needs from its neighbours. */
export interface ReconnectDeps extends Pick<RendererServices, 'shell'> {
  /** The main process. */
  bridge: ReconnectBridge;
  /** The account dialog: closed as this opens, told the saved settings. */
  dialog: Pick<ShellDialogViewModel, 'close' | 'settingsSaved'>;
  /** The welcome screen, which says why the settings could not be read. */
  setup: Pick<SetupViewModel, 'showError'>;
}

/** The reconnect dialog. */
export class ReconnectViewModel extends ViewModel<ReconnectState> {
  /** Its neighbours. */
  private readonly deps: ReconnectDeps;
  /** Reports a failed connection after the wait. */
  private timer: ReturnType<typeof setTimeout> | undefined;

  /** Closed, closing itself once connected. */
  constructor(deps: ReconnectDeps) {
    super({ open: false, server: '', apiKey: '', name: '', error: '', saving: false, button: TEXT.reconnectIdle });
    this.deps = deps;
    this.own(() => clearTimeout(this.timer));
    this.own(deps.bridge.onWsStatus((status) => status.connected && this.connected()));
  }

  /** "Connection settings…": closes the account dialog and opens this one. */
  async editSettings(): Promise<void> {
    this.deps.dialog.close();
    await this.open();
  }

  /** Shows the dialog over the page, filled with the current settings. */
  async open(): Promise<void> {
    await this.deps.bridge.showOverlay(OVERLAYS.reconnect);
    try {
      this.present(shape<Config>(await this.deps.bridge.getConfig()));
    } catch (error) {
      void this.deps.bridge.hideOverlay(OVERLAYS.reconnect);
      this.deps.setup.showError(messageOf(error, TEXT.loadFailed));
    }
  }

  /** A field was typed in. */
  edit(field: ReconnectField, value: string): void {
    this.set({ [field]: value } as Pick<ReconnectState, ReconnectField>);
  }

  /** Hides the dialog and gives the page back. */
  close(): void {
    clearTimeout(this.timer);
    this.set({ open: false });
    void this.deps.bridge.hideOverlay(OVERLAYS.reconnect);
  }

  /** Saves the settings and waits for the connection to report back. */
  async save(): Promise<void> {
    const [server, key, name] = [this.state.server, this.state.apiKey, this.state.name].map((v) => v.trim());
    if (!SERVER_URL.test(server) || !key) return this.set({ error: TEXT.reconnectInvalid });
    clearTimeout(this.timer);
    this.set({ saving: true, button: TEXT.reconnectBusy, error: '' });
    await this.apply(server, key, name);
  }

  /** Saves, then shows the new settings in the account page; or says why it could not. */
  private async apply(server: string, key: string, name: string): Promise<void> {
    try {
      await this.deps.bridge.saveConfig({ serverUrl: server, apiKey: key, browserName: name || undefined });
    } catch (error) {
      return this.set({ error: messageOf(error, TEXT.saveFailed), ...this.idle(TEXT.reconnectRetry) });
    }
    this.saved(server, name);
  }

  /** Fills the form and opens the dialog. */
  private present(config: Config): void {
    const fields = { server: config.serverUrl ?? '', apiKey: config.apiKey ?? '', name: config.browserName ?? '' };
    this.set({ ...fields, error: '', ...this.idle(TEXT.reconnectIdle), open: true });
  }

  /** The save button, enabled, reading `button`. */
  private idle(button: string): Pick<ReconnectState, 'saving' | 'button'> {
    return { saving: false, button };
  }

  /** Saved: the account page shows the new settings, and the wait starts. */
  private saved(server: string, name: string): void {
    this.deps.dialog.settingsSaved(server, name);
    this.timer = setTimeout(() => this.timedOut(), C.CONNECT_TIMEOUT_MS);
  }

  /** No connection after the wait: offer a retry. */
  private timedOut(): void {
    this.set(this.idle(TEXT.reconnectRetry));
    if (!this.deps.shell.state.connected) this.set({ error: TEXT.reconnectTimedOut });
  }

  /** Connected: reset the form and close the dialog if it is open. */
  private connected(): void {
    clearTimeout(this.timer);
    this.set(this.idle(TEXT.reconnectIdle));
    if (this.state.open) this.close();
  }
}
