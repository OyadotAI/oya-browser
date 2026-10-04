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

/** The services deep links use. */
type Deps = Pick<AppServices, 'electron' | 'shell' | 'config' | 'socket' | 'mirror'>;

/** Logs a link that could not be applied. */
const reportLinkFailure = (e: Error): void => console.error('[deeplink]', e.message);

/** Receives oya:// links from every route the OS delivers them by. */
export class DeepLinks {
  /** Links that arrived before the window existed. */
  readonly pending: string[] = [];
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Pair from an oya:// link (see src/main/connection/pairing.ts), then reconnect as the paired browser. */
  async applyDeepLink(rawUrl: string): Promise<boolean> {
    const paired = await pairFromLink(rawUrl, (opts) => this.ask(opts));
    if (!paired) return false;
    this.retarget(paired);
    // The import needs the new connection, so it runs once that signs in.
    if (paired.importLogins) this.deps.mirror.importOnConnect = true;
    // connect() emits ws-status, which is how the renderer learns about this.
    this.deps.socket.connect();
    this.deps.shell.window?.show();
    return true;
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
    if (config.apiKey !== apiKey || config.serverUrl !== serverUrl) this.deps.socket.browserId = null;
    this.deps.socket.disconnect();
    Object.assign(config, { serverUrl, apiKey, persona, signedOut: false, keyFromApp: true });
    this.deps.config.save();
  }

  /** Windows and Linux: the link is an argv entry of a second launch. */
  onSecondInstance(argv: string[]): void {
    const link = argv.find((a) => a.startsWith(LINK_SCHEME));
    if (link) this.applyDeepLink(link).catch(reportLinkFailure);
    this.deps.shell.window?.show();
  }

  /** macOS delivers it as an event, which can fire before the app is ready. */
  onOpenUrl(event: Pick<Event, 'preventDefault'>, url: string): void {
    event.preventDefault();
    if (this.deps.shell.window) this.applyDeepLink(url).catch(reportLinkFailure);
    else this.pending.push(url);
  }

  /** Once the window exists: the links that were waiting, then any on the command line. */
  async drain(argv: string[]): Promise<void> {
    const queued = this.pending.splice(0).concat(argv.filter((a) => a.startsWith(LINK_SCHEME)));
    for (const link of queued) await this.applyDeepLink(link).catch(reportLinkFailure);
  }
}
