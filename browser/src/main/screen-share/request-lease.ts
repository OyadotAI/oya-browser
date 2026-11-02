/** A pending picker may follow external focus, but never a replacement document, owner or profile. */
import type { BrowserWindow, WebContents, WebContentsDidStartNavigationEventParams } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Tab } from '../tabs/types.ts';

/** Main-only ownership predicates supplement, never replace, MediaGate's initial admission. */
export type ShareLeaseDeps = Pick<AppServices, 'electron' | 'windows' | 'agentTab' | 'persona' | 'governance'>;

/** Revocation is latched, including a brief claim or tab switch that returns before the native result. */
export class ShareRequestLease {
  /** The originally admitted page and native frame/session never follow URL-equivalent replacements. */
  private readonly page: WebContents;
  /** Native frame at admission. */
  private readonly identity: {
    /** Immutable native frame at request admission. */
    frame: WebContents['mainFrame'];
    /** Cookie/profile session cannot be replaced during native selection. */
    session: WebContents['session'];
    /** Original profile transition fences away-and-back changes. */
    epoch: symbol;
    /** Exact partition at admission. */
    partition: string;
    /** Recipient identity survives path-only changes, never cross-origin navigation. */
    origin: string;
  };
  /** Actual window scope, independent of the root facade's later routing. */
  private readonly owner: AppServices | undefined;
  /** Exact tab object; recycled ids and transferred views never inherit approval. */
  private readonly tab: Tab | undefined;
  /** The exact original native owner, even if its scope later creates another window. */
  private readonly window: BrowserWindow | undefined;
  /** Native ownership services and current control state. */
  private readonly deps: ShareLeaseDeps;
  /** Original MediaGate generation predicate remains required. */
  private readonly guard: () => boolean;
  /** Once revoked, no later state change can reauthorize this request. */
  private revoked = false;
  /** Every installed observer belongs to this bounded picker operation. */
  private readonly cleanups: (() => void)[] = [];
  /** Snapshot the actual human requester after MediaGate admits it. */
  constructor(deps: ShareLeaseDeps, page: WebContents, guard: () => boolean) {
    this.deps = deps;
    this.page = page;
    this.guard = guard;
    this.identity = shareIdentity(deps, page);
    this.owner = deps.windows?.ownerOfContents(page);
    this.tab = this.owner?.tabs.list.find((tab) => tab.view.webContents === page);
    this.window = this.owner?.shell.window ?? undefined;
    this.watch();
  }
  /** Synchronous control-change callbacks catch brief takeovers before release can hide them. */
  readonly sync = (): void => {
    this.current();
  };
  /** A destroyed native object cannot escape as an exception or be mistaken for a current source. */
  readonly current = (): boolean => {
    try {
      this.revoked ||= !this.valid();
    } catch {
      this.revoked = true;
    }
    return !this.revoked;
  };
  /** External application focus is deliberately absent; ownership and the selected Oya tab remain fixed. */
  private valid(): boolean {
    const { owner, tab, page, deps } = this;
    if (!owner || !tab || tab.window || !this.window || this.window.isDestroyed()) return false;
    if (owner.shell.window !== this.window || !this.identityCurrent()) return false;
    if (deps.windows?.ownerOfContents(page) !== owner || owner.tabs.find(tab.id) !== tab) return false;
    if (owner.tabs.activeTabId !== tab.id || tab.view.webContents !== page || !deps.agentTab.humanMay(tab.id))
      return false;
    return !deps.governance.configuration && this.guard();
  }
  /** Native lifetime and profile changes never reuse the original document's continuation lease. */
  private identityCurrent(): boolean {
    const { page, identity } = this;
    const sameNative = page.mainFrame === identity.frame && page.session === identity.session;
    return !page.isDestroyed() && sameNative && this.profileCurrent();
  }
  /** Profile transitions and recipient identity remain fixed even if native focus moves away. */
  private profileCurrent(): boolean {
    const { page, identity, deps } = this;
    return (
      deps.persona.policyEpoch === identity.epoch &&
      deps.persona.partitionName() === identity.partition &&
      URL.parse(page.getURL())?.origin === identity.origin
    );
  }
  /** Native document/window lifetimes and tab selection are observed without website-controlled messages. */
  private watch(): void {
    this.page.on('did-start-navigation', this.navigate);
    this.page.once('destroyed', this.cancel);
    this.window?.once('closed', this.cancel);
    this.deps.electron.app.on('browser-window-focus', this.focused);
    const unsubscribe = this.deps.windows?.tabEvents.subscribe(this.sync);
    if (unsubscribe) this.cleanups.push(unsubscribe);
  }
  /** A full document navigation revokes even if it later fails or returns to the same URL. */
  private readonly navigate = (event: WebContentsDidStartNavigationEventParams): void => {
    if (event.isMainFrame && !event.isSameDocument) this.cancel();
  };
  /** Picker/helper focus is harmless; selecting another real Oya window ends this pending request. */
  private readonly focused = (_event: unknown, window: BrowserWindow): void => {
    const windows = this.deps.windows;
    const scope = windows?.fromContents(window.webContents) ?? windows?.ownerOfContents(window.webContents);
    if (scope && window !== this.window) this.cancel();
  };
  /** Native callbacks can only revoke a request, never silently restart it. */
  private readonly cancel = (): void => {
    this.revoked = true;
  };
  /** No request-scoped native listener survives success, cancellation or a failed picker asset. */
  dispose(): void {
    this.cancel();
    this.page.removeListener('did-start-navigation', this.navigate);
    this.page.removeListener('destroyed', this.cancel);
    this.window?.removeListener('closed', this.cancel);
    this.deps.electron.app.removeListener('browser-window-focus', this.focused);
    for (const cleanup of this.cleanups.splice(0)) cleanup();
  }
}

/** Snapshot immutable native/profile identity in one place before any asynchronous picker work. */
function shareIdentity(deps: ShareLeaseDeps, page: WebContents) {
  return {
    frame: page.mainFrame,
    session: page.session,
    epoch: deps.persona.policyEpoch,
    partition: deps.persona.partitionName(),
    origin: URL.parse(page.getURL())?.origin ?? '',
  };
}
