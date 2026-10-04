/**
 * The account card at the top of the account page: who this browser is
 * signed in as (avatar, name or email, project and plan), whether it is
 * connected, and the Log out and Switch account actions.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { ConnectionStatus } from '../../../../shared/ipc.ts';
import { shapeOrNull, type Account, type Config } from '../model/models.ts';

/** The connection as the card says it. */
export type AccountConnection = 'connected' | 'reconnecting' | 'offline';

/** What the account card knows. */
export interface AccountState {
  /** The settings in effect (server, whether a key is saved). */
  config: Config;
  /** Whether the control socket is connected. */
  connected: boolean;
  /** The account last shown, or null when only a key is known. */
  account: Account | null;
  /** Whether the account page is on screen (it learns the account once the server can answer). */
  visible: boolean;
}

/** What the card shows, worked out from its state. */
export interface AccountCard {
  /** The heading: the person's name or email, or that a key signs this browser in. */
  title: string;
  /** The email under a name ('' to hide it). */
  email: string;
  /** The avatar's letter. */
  initial: string;
  /** "Project Lab", or what to do to see it ('' for nothing). */
  project: string;
  /** "pro plan" ('' when the server has no plans). */
  plan: string;
  /** The connection. */
  connection: AccountConnection;
  /** The connection in words. */
  connectionText: string;
  /** Log out's label and tooltip. */
  signOut: SignOutLabel;
}

/** Log out's label and tooltip. */
export interface SignOutLabel {
  /** "Log out of ada@example.com". */
  text: string;
  /** What it does. */
  title: string;
}

/** The parts of the bridge the card uses. */
export type AccountBridge = Pick<OyaBrowser, 'getAccount' | 'openConsole' | 'signOut' | 'onWsStatus'>;

/** The server's host, as people read it. */
export function hostOf(serverUrl: string | undefined): string {
  try {
    return new URL(String(serverUrl).replace(/^ws/, 'http')).host;
  } catch {
    return 'the server';
  }
}

/** Who, in words: the person's name over their email, or that a key alone signs this browser in. */
function who(account: Account | null): Pick<AccountCard, 'title' | 'email' | 'initial'> {
  const label = account?.name || account?.email;
  if (label) return { title: label, email: account?.name ? account.email || '' : '', initial: label[0] };
  const initial = account?.project?.name?.[0] || 'O';
  return { title: 'Signed in with an API key', email: '', initial };
}

/** Connected, reconnecting (a key is saved, the socket keeps trying) or offline, with the host. */
function connection(state: AccountState): Pick<AccountCard, 'connection' | 'connectionText'> {
  const host = hostOf(state.config.serverUrl);
  if (state.connected) return { connection: 'connected', connectionText: `Connected to ${host}` };
  if (state.config.apiKey) return { connection: 'reconnecting', connectionText: `Reconnecting to ${host}…` };
  return { connection: 'offline', connectionText: 'Offline' };
}

/** Log out names the account it signs out of, when it is known. */
function signOutLabel(email: string | null | undefined): SignOutLabel {
  if (!email) return { text: 'Log out', title: "Forget this browser's key" };
  return { text: `Log out of ${email}`, title: `Forget this browser's key and sign out of ${email}` };
}

/** The card for `state`. */
export function accountCard(state: AccountState): AccountCard {
  const { account } = state;
  const name = account?.project?.name;
  const project = name ? `Project ${name}` : state.connected ? '' : 'Connect to see your project';
  const base = { ...who(account), project, plan: account?.plan ? `${account.plan} plan` : '' };
  const initial = base.initial.toUpperCase();
  return { ...base, initial, ...connection(state), signOut: signOutLabel(account?.email) };
}

/** The account card and its actions. */
export class AccountViewModel extends ViewModel<AccountState> {
  /** The main process. */
  private readonly bridge: AccountBridge;

  /** Knows nothing until loaded, following the connection. */
  constructor(bridge: AccountBridge) {
    super({ config: {}, connected: false, account: null, visible: false });
    this.bridge = bridge;
    this.own(bridge.onWsStatus((status) => this.onStatus(status)));
  }

  /** Takes the settings and the live status, and asks who this browser is signed in as. */
  async load(config: Config, status: ConnectionStatus): Promise<void> {
    this.set({ config, connected: !!status.connected });
    this.set({ account: await this.fetch() });
  }

  /** The settings changed (a new name, a new server). */
  setConfig(config: Config): void {
    this.set({ config });
  }

  /** The account page came on screen or left it. */
  setVisible(visible: boolean): void {
    this.set({ visible });
  }

  /** The connection came or went: learn the account once the server can answer. */
  onStatus(status: ConnectionStatus): void {
    this.set({ connected: !!status.connected });
    if (!status.connected || this.state.account || !this.state.visible) return;
    void this.fetch().then((account) => this.set({ account }));
  }

  /** Opens the console's connect page: signing in there as someone else links this browser to them. */
  switchAccount(): void {
    void this.bridge.openConsole(this.state.config.serverUrl);
  }

  /** Forgets this browser's key. */
  signOut(): void {
    void this.bridge.signOut();
  }

  /** The account, or null when it cannot be learned. */
  private async fetch(): Promise<Account | null> {
    return shapeOrNull<Account>(await this.bridge.getAccount().catch(() => null));
  }
}
