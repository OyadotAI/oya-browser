/** Connection-owned native browser resources: private contexts, downloads and original network streams. */
import type { Session, WebContents } from 'electron';
import type { AppServices } from './services.ts';
import { appNativeContexts } from './native-contexts.ts';
import { nativeCookies } from '../native-cookies/index.ts';
import { NativeDownloads } from '../native-downloads/index.ts';
import { NativeNetworkSession, type NetworkDependencies, type NetworkSink } from '../native-network/index.ts';
/** Resource lifetime is shared only within one authenticated front-door connection. */
export class AppNativeBrowser {
  /** Owned ephemeral partitions. */
  readonly contexts: ReturnType<typeof appNativeContexts>;
  /** Native requests for exclusively owned sessions. */
  private readonly networks = new Map<Session, NativeNetworkSession>();
  /** Downloads cannot mutate the default-profile session. */
  private readonly downloads: NativeDownloads;
  /** Current human-control check also guards context-wide operations without a page. */
  private readonly cookieAllowed: () => boolean;
  /** Compose native services with current control/target policy. */
  constructor(deps: AppServices, policy: NetworkDependencies) {
    this.cookieAllowed = () =>
      !deps.control.busy && !deps.control.localHeld && (!deps.control.connected || deps.control.state.mode === 'agent');
    this.downloads = new NativeDownloads(policy);
    const lifecycle = {
      install: (session: Session) => this.networks.set(session, new NativeNetworkSession(session, policy)),
      remove: (session: Session) => this.removeNetwork(session),
    };
    this.contexts = appNativeContexts(deps, { downloads: this.downloads, network: lifecycle });
  }
  /** Teardown cancels held network callbacks before session storage is cleared. */
  private removeNetwork(session: Session): void {
    this.networks.get(session)?.dispose();
    this.networks.delete(session);
  }
  /** No fallback installs session-wide listeners on the person's ordinary browser profile. */
  network(contents: WebContents): NativeNetworkSession {
    const id = this.contexts.id(contents.session);
    if (!id) throw Error('Native network control requires an owned ephemeral browser context');
    this.contexts.get(id);
    const network = this.networks.get(contents.session);
    if (!network) throw Error('Native network context unavailable');
    return network;
  }
  /** Browser management keeps explicit context validation inside the native resource owner. */
  async manage(action: string, params: Record<string, unknown>, emit: NetworkSink): Promise<object> {
    if (action.startsWith('cookies:')) return nativeCookies(() => this.cookieSession(params), action, params);
    if (action.startsWith('download:')) return this.download(action, params, emit);
    if (action === 'context:create') return { browserContextId: await this.contexts.create() };
    if (action === 'context:list') return { browserContextIds: this.contexts.list() };
    if (action !== 'context:dispose') throw Error('Unsupported native browser operation');
    await this.contexts.remove(params.browserContextId as string);
    return {};
  }
  /** Revocation during a native await prevents export or later batch writes. */
  private cookieSession(params: Record<string, unknown>): Session {
    if (!this.cookieAllowed()) throw Error('Native cookie access is no longer authorized');
    return this.contexts.get(params.browserContextId as string);
  }
  /** Destination changes and cancellation can address only a live owned private context. */
  private download(action: string, params: Record<string, unknown>, emit: NetworkSink): object {
    const session = this.contexts.get(params.browserContextId as string);
    if (action === 'download:set') this.downloads.set(session, params, emit);
    else if (action === 'download:cancel') this.downloads.cancel(session, params.guid as string);
    else throw Error('Unsupported native download operation');
    return {};
  }
}
