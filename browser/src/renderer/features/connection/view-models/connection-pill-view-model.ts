/**
 * The toolbar's connection pill: connected (with the browser id on hover),
 * connecting (a key is saved, the first status has not come), or offline.
 * It also reads the status at start, which switches the shell to browsing,
 * and opens the account page when clicked.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { RendererServices } from '../../../app/services.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { ConnectionStatus, ShellStatus } from '../../../../shared/ipc.ts';
import { shape, type Config } from '../model/models.ts';

/** What the pill shows. */
export interface ConnectionPillState {
  /** The pill's classes: 'conn-pill', plus 'ok' or 'trying'. */
  className: string;
  /** Its words. */
  label: string;
  /** Its tooltip. */
  title: string;
}

/** The parts of the bridge the pill uses. */
export type ConnectionPillBridge = Pick<OyaBrowser, 'getConfig' | 'getStatus' | 'onWsStatus'>;

/** Opens the account page (the shell dialog). */
export interface AccountOpener {
  /** Opens the dialog on the account page when `profile`, else on the commands. */
  open(profile?: boolean): Promise<void>;
}

/** The status assumed when it cannot be read. */
const OFFLINE: Partial<ShellStatus> & ConnectionStatus = { connected: false };

/** The pill for a connection status. */
export function pillFor(status: ConnectionStatus): ConnectionPillState {
  if (!status.connected && status.failure)
    return { className: 'conn-pill', label: 'Connection failed', title: status.failure };
  if (!status.connected)
    return { className: 'conn-pill', label: 'Offline', title: 'Not connected, click to configure' };
  const id = status.browserId ? `, ${status.browserId}` : '';
  return { className: 'conn-pill ok', label: 'Connected', title: `Connected${id}` };
}

/** What the pill needs from its neighbours. */
export interface ConnectionPillDeps extends Pick<RendererServices, 'shell'> {
  /** The main process. */
  bridge: ConnectionPillBridge;
  /** The account page the pill opens. */
  account: AccountOpener;
}

/** The connection pill. */
export class ConnectionPillViewModel extends ViewModel<ConnectionPillState> {
  /** The main process. */
  private readonly bridge: ConnectionPillBridge;
  /** Connected and mode, which the status at start sets. */
  private readonly shell: RendererServices['shell'];
  /** The account page. */
  private readonly account: AccountOpener;
  /** A status has been shown, so "Connecting…" is no longer news. */
  private heard = false;

  /** Offline until told, following the connection. */
  constructor(deps: ConnectionPillDeps) {
    super(pillFor({ connected: false }));
    this.bridge = deps.bridge;
    this.shell = deps.shell;
    this.account = deps.account;
    this.own(this.bridge.onWsStatus((status) => this.show(status)));
    void this.load();
  }

  /** A configured browser says "Connecting…" until the status arrives; the status at start sets the mode. */
  async load(): Promise<void> {
    const config = shape<Config>(await this.bridge.getConfig().catch(() => null));
    if (config.apiKey && !this.heard) this.set({ className: 'conn-pill trying', label: 'Connecting…' });
    const status = await this.bridge.getStatus().catch(() => OFFLINE);
    if (status.browsing) this.shell.setMode('browsing');
    this.show(status);
  }

  /** The pill was clicked: the account page. */
  click(): void {
    void this.account.open(true);
  }

  /** Shows a connection status. */
  private show(status: ConnectionStatus): void {
    this.heard = true;
    this.shell.setConnected(!!status.connected);
    this.set(pillFor(status));
  }
}
