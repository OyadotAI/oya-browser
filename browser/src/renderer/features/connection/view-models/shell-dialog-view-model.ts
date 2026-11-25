/**
 * The shell dialog: the command palette, or the account page (who this
 * browser is signed in as, sync, imports, the device, and the server and
 * browser id under Advanced). Opening the account page reads the settings and
 * status afresh, since a pairing link or a reconnect can change them.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { ConnectionStatus, ShellStatus } from '../../../../shared/ipc.ts';
import { OVERLAYS, TEXT } from '../model/constants.ts';
import { shape, type Config } from '../model/models.ts';
import type { AccountViewModel } from './account-view-model.ts';
import type { SyncViewModel } from './sync-view-model.ts';
import type { ImportViewModel } from './import-view-model.ts';
import type { ProfileViewModel } from './profile-view-model.ts';

/** The dialog's pages. */
export type DialogPage = 'commands' | 'profile' | 'shortcuts';

/** Independent sections within Settings. */
export type SettingsPage = 'general' | 'logins' | 'account';

/** What the dialog shows. */
export interface ShellDialogState {
  /** The dialog is open. */
  open: boolean;
  /** The page in view. */
  page: DialogPage;
  /** The selected Settings section. */
  settingsPage: SettingsPage;
  /** The saved server address ('' when none). */
  server: string;
  /** This browser's id on the server ('' while offline). */
  browserId: string;
  /** Advanced is unfolded. */
  advanced: boolean;
  /** What the last Copy said. */
  copyStatus: string;
}

/** The parts of the bridge the dialog uses. */
export type ShellDialogBridge = Pick<
  OyaBrowser,
  'showOverlay' | 'hideOverlay' | 'getConfig' | 'getStatus' | 'onWsStatus' | 'onModeChanged'
>;

/** Copies text (navigator.clipboard in the page). */
export interface Clipboard {
  /** Puts `text` on the clipboard. */
  writeText(text: string): Promise<void>;
}

/** The account page's parts, which the dialog fills when it opens. */
export interface AccountParts {
  /** The account card. */
  account: Pick<AccountViewModel, 'load' | 'setConfig' | 'onStatus'>;
  /** Sync. */
  sync: Pick<SyncViewModel, 'load' | 'onStatus'>;
  /** Imported logins. */
  imports: Pick<ImportViewModel, 'showHistory' | 'onStatus'>;
  /** Browsing as, and this device. */
  profile: Pick<ProfileViewModel, 'loadConfig' | 'onStatus'>;
}

/** What the dialog's header and label say. */
export interface PageLabels {
  /** The heading. */
  title: string;
  /** The dialog's aria-label. */
  label: string;
}

/** What the dialog needs from its neighbours. */
export interface ShellDialogDeps extends AccountParts {
  /** The main process. */
  bridge: ShellDialogBridge;
  /** The clipboard. */
  clipboard: Clipboard;
}

/** What the dialog's header and label say for a page. */
export function pageLabels(page: DialogPage): PageLabels {
  if (page === 'shortcuts') return { title: 'Keyboard shortcuts', label: 'Keyboard shortcuts' };
  return page === 'profile'
    ? { title: 'Settings', label: 'Settings' }
    : { title: 'Commands', label: 'Commands and settings' };
}

/** Initial settings navigation is independent of account and connection data. */
const INITIAL_STATE: ShellDialogState = {
  open: false,
  page: 'commands',
  settingsPage: 'general',
  server: '',
  browserId: '',
  advanced: false,
  copyStatus: '',
};

/** The commands and account dialog. */
export class ShellDialogViewModel extends ViewModel<ShellDialogState> {
  /** The main process. */
  private readonly bridge: ShellDialogBridge;
  /** The clipboard. */
  private readonly clipboard: Clipboard;
  /** The account page's parts. */
  private readonly parts: AccountParts;

  /** Closed, then fills the account page from the settings, status and account at start. */
  constructor(deps: ShellDialogDeps) {
    super(INITIAL_STATE);
    this.bridge = deps.bridge;
    this.clipboard = deps.clipboard;
    this.parts = deps;
    this.own(this.bridge.onWsStatus((status) => this.set({ browserId: status.browserId || '' })));
    this.own(this.bridge.onModeChanged((mode) => mode !== 'browsing' && this.close()));
    void this.load();
  }

  /** The settings and status at start: the app may have connected before this page was listening. */
  async load(): Promise<void> {
    const reads = Promise.all([this.bridge.getConfig(), this.bridge.getStatus()]);
    const [config, status] = await reads.catch(() => [{}, { connected: false }] as const);
    this.loadConfig(shape<Config>(config));
    this.parts.imports.showHistory(shape<Config>(config).imports);
    this.onStatus(status);
    // The toolbar avatar and profile menu name the account from launch, online or not: main answers
    // with the one it remembers until the server can say.
    await this.parts.account.load(shape<Config>(config), status);
  }

  /** Opens the dialog on the account page, or on the command palette. */
  async open(profile = false, settingsPage: SettingsPage = 'general'): Promise<void> {
    if (profile) await this.refresh().catch(() => {}); // the page still opens, with what it last knew
    await this.bridge.showOverlay(OVERLAYS.shell);
    this.set({ open: true, page: profile ? 'profile' : 'commands', settingsPage });
  }

  /** Opens the learning guide using the same native-view-safe overlay. */
  async openShortcuts(): Promise<void> {
    await this.bridge.showOverlay(OVERLAYS.shell);
    this.set({ open: true, page: 'shortcuts' });
  }

  /** Hides the dialog and gives the page back. */
  close(): void {
    if (!this.state.open) return;
    this.set({ open: false });
    void this.bridge.hideOverlay(OVERLAYS.shell);
  }

  /** Changes Settings sections without reloading data or closing the overlay. */
  chooseSettings(settingsPage: SettingsPage): void {
    this.set({ settingsPage });
  }

  /** Advanced was folded or unfolded. */
  setAdvanced(advanced: boolean): void {
    this.set({ advanced });
  }

  /** The reconnect dialog saved new settings: show them. */
  settingsSaved(server: string, name: string): void {
    this.set({ server });
    if (name) this.parts.profile.loadConfig({ browserName: name });
  }

  /** Copies `text`, saying what was copied, or how to copy it by hand. */
  async copy(what: 'browser ID' | 'server address'): Promise<void> {
    const text = what === 'browser ID' ? this.state.browserId : this.state.server || TEXT.noServer;
    const done = await this.clipboard.writeText(text).then(
      () => `${what[0].toUpperCase()}${what.slice(1)} copied.`,
      () => `Could not copy. Select the ${what} to copy it manually.`,
    );
    this.set({ copyStatus: done });
  }

  /** Reads the settings, status and account again, so the page says what is true now. */
  private async refresh(): Promise<void> {
    const [raw, status] = await Promise.all([this.bridge.getConfig(), this.bridge.getStatus()]);
    const config = shape<Config>(raw);
    this.loadConfig(config);
    this.set({ advanced: false, browserId: status.browserId || '' });
    this.parts.imports.showHistory(config.imports);
    this.parts.sync.load(config, status);
    await this.parts.account.load(config, status);
  }

  /** The saved server and this browser's name. */
  private loadConfig(config: Config): void {
    this.set({ server: config.serverUrl || '' });
    this.parts.profile.loadConfig(config);
    this.parts.account.setConfig(config);
  }

  /** A status for every part of the page. */
  private onStatus(status: ShellStatus | ConnectionStatus): void {
    this.set({ browserId: status.browserId || '' });
    for (const part of [this.parts.account, this.parts.sync, this.parts.imports, this.parts.profile])
      part.onStatus(status);
  }
}
