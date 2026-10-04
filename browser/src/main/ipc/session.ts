/** IPC: settings, connection status, control handoff, the saved profile and the fingerprint. */
import type { AppServices } from '../app/services.ts';
import type { ConfigValues } from '../app/config-store.ts';
import type { ControlChange, PersonaList, ShellStatus } from '../../shared/ipc.ts';
import type { HandlersOf } from './handle.ts';
import { ServerApi } from '../connection/server-api.ts';
import { pairingServer } from '../connection/pairing.ts';

/** The services the session handlers use. */
type Deps = Pick<
  AppServices,
  'config' | 'socket' | 'cookies' | 'persona' | 'tabs' | 'shell' | 'control' | 'mirror' | 'electron'
>;

/** The channels this group answers. */
type Channel =
  | 'get-control-state'
  | 'change-control'
  | 'get-config'
  | 'open-console'
  | 'save-config'
  | 'get-status'
  | 'save-profile'
  | 'import-sources'
  | 'reimport-browser'
  | 'sign-out'
  | 'get-fingerprint'
  | 'list-personas'
  | 'get-account';

/** A persona as the server lists it. */
interface ListedPersona {
  /** Its id. */
  id: string;
  /** Its display name. */
  name: string;
  /** Whether it is the project's default. */
  isDefault?: boolean;
}

/** GET /api/personas. */
interface PersonasAnswer {
  /** The project's personas. */
  personas: ListedPersona[];
}

/** The settings a save leaves: a move to another project starts the persona and account fresh, a typed key wins from then on. */
function savedSettings(changes: Partial<ConfigValues>, moved: boolean): Partial<ConfigValues> {
  const chosen = 'apiKey' in changes ? { keyFromApp: true } : {};
  const fresh = moved ? { persona: 'default', account: null } : {};
  return { ...fresh, ...changes, signedOut: false, ...chosen };
}

/**
 * The workspace's own console, derived from the server address rather than
 * taken from the renderer: openExternal will hand any scheme to the operating
 * system, so only a ws or wss address becomes a link, and only its origin.
 */
export function consoleUrl(serverUrl: string): string | null {
  try {
    const url = new URL(serverUrl);
    if (!['ws:', 'wss:'].includes(url.protocol)) return null;
    return `${url.protocol === 'wss:' ? 'https:' : 'http:'}//${url.host}/dashboard?connect=desktop`;
  } catch {
    return null;
  }
}

/** Whether `changes` point this browser at another key or server, that is another project. */
function movesProject(config: ConfigValues, changes: Partial<ConfigValues>): boolean {
  const differs = (field: 'apiKey' | 'serverUrl'): boolean => field in changes && changes[field] !== config[field];
  return differs('apiKey') || differs('serverUrl');
}

/** `changes`, refused when its server URL is one the renderer should not have let through: plaintext ws:// would send the key and cookies in the clear. */
function checkedServer(changes: Partial<ConfigValues>): Partial<ConfigValues> {
  if ('serverUrl' in changes && !pairingServer(String(changes.serverUrl))) {
    throw new Error('Use wss://, or ws:// to this machine');
  }
  return changes;
}

/** Settings, the connection, control and the persona, as the shell asks for them. */
export class SessionHandlers {
  /** Channel → handler. */
  readonly handlers: HandlersOf<Channel> = {
    'get-control-state': () => this.deps.control.snapshot(),
    'change-control': (_e, action) => this.changeControl(action),
    'get-config': () => this.deps.config.values,
    'open-console': (_e, serverUrl) => this.openConsole(serverUrl),
    'save-config': (_e, changes) => this.saveConfig(checkedServer(changes)),
    'get-status': () => this.status(),
    'save-profile': () => this.saveProfile(),
    'import-sources': () => this.deps.mirror.sources(),
    // The id comes from the shell page: only a listed browser is ever opened.
    'reimport-browser': (_e, sourceId) => this.deps.mirror.reimport(this.listedSource(sourceId)),
    'sign-out': () => this.signOut(),
    'get-fingerprint': () => this.deps.persona.summary(),
    'list-personas': () => this.listPersonas(),
    'get-account': () => this.readAccount(),
  };
  /** The main-process services. */
  private readonly deps: Deps;
  /** The server's API, as this browser. */
  private readonly api: ServerApi;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
    this.api = new ServerApi(deps);
  }

  /** Takes or hands back control; a refusal answers why, with the state as it stands. */
  private async changeControl(action: string): Promise<ControlChange> {
    try {
      return { state: await this.deps.control.change(action) };
    } catch (error) {
      return { error: (error as Error).message, state: this.deps.control.snapshot() };
    }
  }

  /** Opens the server's console in the system browser; answers its address, or null for a server that has none. */
  private async openConsole(serverUrl: string | undefined): Promise<string | null> {
    const url = consoleUrl(serverUrl || this.deps.config.values.serverUrl);
    if (url) await this.deps.electron.shell.openExternal(url);
    return url;
  }

  /** The connection, the active address and when cookies last synced. */
  private status(): ShellStatus {
    return {
      connected: this.deps.socket.ready,
      browserId: this.deps.socket.browserId,
      profileName: this.deps.config.values.profileName,
      url: this.deps.tabs.getActiveView()?.webContents.getURL() || '',
      // The renderer asks for this after it loads. `mode-changed` is sent once, and
      // a shell that was still loading when the server accepted the browser would
      // otherwise sit on the setup screen for the rest of the session.
      browsing: !!this.deps.shell.browsingMode,
      // When this browser's logins last went to the server (0 before they have this run).
      syncedAt: this.deps.cookies?.syncedAt?.() || 0,
    };
  }

  /** Pushes this browser's cookies and storage to the server, now. */
  private async saveProfile(): Promise<void> {
    const { socket, cookies } = this.deps;
    if (!socket.ready || !socket.isOpen()) throw new Error('Connect the desktop before saving your profile.');
    cookies.flushCookieChanges();
    await cookies.dumpCookies();
    await this.deps.persona.flushJar();
    socket.send({ type: 'profile_flush' });
  }

  /**
   * Logs out of Oya: forgets the key and goes back to the welcome screen. The
   * server address, browser name and the sites this browser is logged in to stay.
   * `signedOut` keeps an OYA_API_KEY in the environment from signing it back in
   * on the next launch; signing in again clears it.
   */
  private signOut(): void {
    this.deps.socket.disconnect();
    this.deps.socket.browserId = null;
    this.deps.config.merge({ apiKey: '', signedOut: true, keyFromApp: false, account: null });
    this.deps.config.save();
    this.deps.tabs.leaveBrowsingMode();
  }

  /** What is known of the account when the server cannot say: nothing, or what it said last time. */
  private lastAccount(): Record<string, unknown> | null {
    return this.deps.config.values.account || null;
  }

  /**
   * Who this browser is signed in as (GET /api/auth/whoami: email, name, plan,
   * project), asked of the server while connected and remembered, so the profile
   * dialog can still say it offline. A server too old to answer leaves what was known.
   */
  private async readAccount(): Promise<Record<string, unknown> | null> {
    if (!this.deps.socket.ready) return this.lastAccount();
    const account = (await this.api.get('auth/whoami').catch(() => null)) as Record<string, unknown> | null;
    if (!account) return this.lastAccount();
    this.deps.config.merge({ account });
    this.deps.config.save();
    return account;
  }

  /** The project's personas for the chat's profile picker, and the one this browser asked for. */
  private async listPersonas(): Promise<PersonaList> {
    const active = this.deps.config.values.persona || 'default';
    if (!this.deps.socket.ready) return { personas: [], active };
    const { personas } = (await this.api.get('personas')) as PersonasAnswer;
    return { personas: personas.map(({ id, name, isDefault }) => ({ id, name, isDefault })), active };
  }

  /**
   * Saves the settings and reconnects. A key typed here is the person's choice,
   * kept over OYA_API_KEY from then on. Another project's server refused the old
   * session id and persona (a foreign id, an unknown persona) on every retry, so
   * a move starts both fresh, as a pairing link does.
   */
  private saveConfig(changes: Partial<ConfigValues>): boolean {
    const moved = movesProject(this.deps.config.values, changes);
    if (moved) this.deps.socket.browserId = null;
    this.deps.config.merge(savedSettings(changes, moved));
    this.deps.config.save();
    this.deps.socket.disconnect();
    this.deps.socket.connect();
    return true;
  }

  /** `sourceId` when it names a browser the import lists; undefined (the default browser) otherwise. */
  private listedSource(sourceId: string): string | undefined {
    return this.deps.mirror.sources().some((source) => source.id === sourceId) ? sourceId : undefined;
  }
}
