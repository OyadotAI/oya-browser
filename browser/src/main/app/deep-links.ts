/**
 * One-click sign-in (oya:// deep links).
 *
 * The dashboard hands out `oya://connect?key=...&server=...` so a customer can
 * go from "pick Oya Browsers" to a signed-in desktop browser without copying a
 * key by hand. Cookies gathered here are what the remote browsers reuse, so
 * this is the step that makes an agent arrive already logged in.
 */
import type { Event, MessageBoxOptions, MessageBoxReturnValue } from 'electron';
import type { AppServices } from './services.ts';
import { pairFromLink, type Paired } from '../connection/pairing.ts';
import { LINK_SCHEME } from './constants.ts';
import { AgentConnectLink, isAgentConnectLink, type Endpoint } from '../local-mcp/index.ts';

/** The services deep links use. */
type Deps = Pick<AppServices, 'electron' | 'shell' | 'config' | 'socket' | 'mirror' | 'tabs' | 'agentTab' | 'localMcp'>;

/** Logs a link that could not be applied. */
const reportLinkFailure = (e: Error): void => console.error('[deeplink]', e.message);

/** Receives oya:// links from every route the OS delivers them by. */
export class DeepLinks {
  /** Links that arrived before the window existed. */
  readonly pending: string[] = [];
  /** The main-process services. */
  private readonly deps: Deps;
  /**
   * The connect-an-agent link's flow. Built here, not in main.ts: it needs only this class's own
   * dialog and window, and the MCP port, which is read when a link arrives because it starts last.
   */
  private readonly agentLink: AgentConnectLink;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
    this.agentLink = new AgentConnectLink({
      ask: (options) => this.ask(options),
      show: () => this.deps.shell.window?.show(),
      endpoint: () => this.mcpEndpoint(),
    });
  }

  /** This app's MCP address and token, or null while its port is not up or not offered. */
  private mcpEndpoint(): Endpoint | null {
    const mcp = this.deps.localMcp;
    return mcp?.token ? { url: mcp.url, token: mcp.token } : null;
  }

  /**
   * Where a link from the OS goes: a connect-an-agent link to its dialog, anything else to pairing
   * or a new tab. Only the OS routes reach this. A link a page inside Oya opens goes to
   * applyDeepLink, which does not know the connect link, so Oya's own tabs can never raise it.
   */
  private route(url: string): Promise<unknown> {
    if (!isAgentConnectLink(url)) return this.applyDeepLink(url);
    this.agentLink.open(url).catch(reportLinkFailure);
    return Promise.resolve(true);
  }

  /** Pair from an oya:// link (see src/main/connection/pairing.ts), then reconnect as the paired browser. */
  async applyDeepLink(rawUrl: string): Promise<boolean> {
    if (/^https?:\/\//i.test(rawUrl)) return this.openWebLink(rawUrl);
    const paired = await pairFromLink(rawUrl, (opts) => this.ask(opts), !!this.deps.config.values.mirroredFrom);
    if (!paired) return false;
    this.retarget(paired);
    // The import needs the new connection, so it runs once that signs in.
    if (paired.importLogins) this.deps.mirror.importOnConnect = true;
    // connect() emits ws-status, which is how the renderer learns about this.
    this.deps.socket.connect();
    this.deps.shell.window?.show();
    return true;
  }

  /** External web links open a new protected tab, never replace the person's active page. */
  private async openWebLink(rawUrl: string): Promise<boolean> {
    const url = new URL(rawUrl);
    if (url.username || url.password) return false;
    // Per tab: with agents connected, the person still owns any new tab.
    if (!this.deps.agentTab.humanMay(null)) return this.blockedWebLink();
    if (this.deps.shell.browsingMode) this.deps.tabs.createTab(url.href, true);
    else this.deps.tabs.enterBrowsingMode(url.href);
    this.deps.shell.window?.show();
    return true;
  }

  /** An OS link must not interrupt an agent-owned tab or bypass the control gate. */
  private async blockedWebLink(): Promise<false> {
    await this.ask({
      message: 'Switch to Human Mode before opening this link',
      detail: 'OyaAI is using the browser. Switch to Human Mode, then open the link again.',
      buttons: ['OK'],
    });
    return false;
  }

  /** showMessageBox refuses a null parent, and second-instance can arrive before the window exists. */
  ask(opts: MessageBoxOptions): Promise<MessageBoxReturnValue> {
    const { dialog } = this.deps.electron;
    const win = this.deps.shell.window;
    return win ? dialog.showMessageBox(win, opts) : dialog.showMessageBox(opts);
  }

  /** Points the saved config at the paired server and drops the old connection. */
  retarget({ apiKey, persona, serverUrl }: Pick<Paired, 'apiKey' | 'persona' | 'serverUrl'>): void {
    const config = this.deps.config.values;
    // A deep link can retarget this browser at a different project (each project
    // has its own key). The durable session id is scoped to a project, so reusing
    // the previous browserId makes the new project reject the socket as a foreign
    // id, close 4003, which the client treats as fatal and never retries, so the
    // desktop sits offline until a full restart happens to mint a fresh id. Rebind
    // to a new session on any key/server change; an unchanged target keeps its id.
    // The remembered account (whoami) and claim page belong to the old key too: drop them, or the
    // account page keeps naming the previous person, and Claim opens their page, after pairing as someone else.
    const moved = config.apiKey !== apiKey || config.serverUrl !== serverUrl;
    if (moved) Object.assign(config, { account: null, claimUrl: '' });
    if (moved) this.deps.socket.browserId = null;
    this.deps.socket.disconnect();
    Object.assign(config, { serverUrl, apiKey, persona, signedOut: false, keyFromApp: true });
    this.deps.config.save();
  }

  /** Windows and Linux: the link is an argv entry of a second launch. */
  onSecondInstance(argv: string[]): void {
    for (const link of argv.filter(isLaunchLink)) this.onOpenUrl({ preventDefault() {} }, link);
    this.deps.shell.window?.show();
  }

  /** macOS delivers it as an event, which can fire before the app is ready. */
  onOpenUrl(event: Pick<Event, 'preventDefault'>, url: string): void {
    event.preventDefault();
    if (this.deps.shell.window) this.route(url).catch(reportLinkFailure);
    else this.pending.push(url);
  }

  /** Once the window exists: the links that were waiting, then any on the command line. */
  async drain(argv: string[]): Promise<void> {
    const queued = this.pending.splice(0).concat(argv.filter(isLaunchLink));
    for (const link of queued) await this.route(link).catch(reportLinkFailure);
  }
}

/** Command-line paths and switches are never treated as web navigation. */
function isLaunchLink(value: string): boolean {
  return value.startsWith(LINK_SCHEME) || /^https?:\/\//i.test(value);
}
