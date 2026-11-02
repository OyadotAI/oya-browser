/** Binds a native account chooser to its real window, human owner, profile and requesting frame. */
import type { BrowserWindow, WebContents, WebFrameMain, WebContentsDidStartNavigationEventParams } from 'electron';
import type { AppServices } from './services.ts';
import type { Tab } from '../tabs/types.ts';
import { PASSKEY_PROMPT_TIMEOUT_MS, PASSKEY_RECHECK_MS } from './constants.ts';

/** Native ownership services; no credential data crosses this boundary. */
export type PasskeyDeps = Pick<AppServices, 'electron' | 'windows' | 'agentTab' | 'persona' | 'governance'>;
/** Popup ancestors retain their exact tracked tab and native window ownership scope. */
interface Relative {
  /** Exact adopted tab object, never a reused numeric id. */
  tab: Tab;
  /** Moving an opener to another window revokes an in-flight choice. */
  owner: AppServices;
}
/** The original native source and identity are immutable throughout a prompt. */
export interface PasskeySource {
  /** Exact tracked page, including a tracked popup. */
  tab: Tab;
  /** Original WebContents, even if the tracked tab later replaces its view. */
  contents: WebContents;
  /** Window scope at admission; tab transfer cannot move consent. */
  owner: AppServices;
  /** Real parent for the account dialog. */
  window: BrowserWindow;
  /** Native frame which asked Chromium for account selection. */
  frame: WebFrameMain;
  /** Original immutable Electron session. */
  session: WebContents['session'];
  /** Profile transition epoch also fences a switch away and back. */
  epoch: symbol;
  /** Exact partition at admission. */
  partition: string;
  /** Popup provenance stays bound to every tracked opener, including nested OAuth windows. */
  family: Relative[];
}

/** Resolve the requesting page rather than the root facade's currently routed tab. */
export function passkeySource(deps: PasskeyDeps, contents: WebContents, frame: WebFrameMain): PasskeySource | null {
  const owner = deps.windows?.ownerOfContents(contents);
  const tab = owner?.tabs.list.find((item) => item.view.webContents === contents);
  const window = tab?.window ?? owner?.shell.window;
  if (!owner || !tab || !window || (tab.window && tab.openerId === undefined)) return null;
  const family = passkeyFamily(deps, tab);
  if (!family) return null;
  const source = { owner, tab, window, frame, contents, family, ...passkeyIdentity(deps, contents) };
  return passkeyCurrent(deps, source) ? source : null;
}

/** Snapshot native session and the current persona transition before showing any account labels. */
function passkeyIdentity(deps: PasskeyDeps, contents: WebContents) {
  return { session: contents.session, epoch: deps.persona.policyEpoch, partition: deps.persona.partitionName() };
}

/** Recheck all mutable boundaries before and after the native dialog. */
export function passkeyCurrent(deps: PasskeyDeps, source: PasskeySource): boolean {
  const { tab, owner, window, frame, contents } = source;
  if (tab.view.webContents !== contents || contents.isDestroyed() || window.isDestroyed()) return false;
  if (frame.isDestroyed() || frame.detached) return false;
  if (deps.governance.configuration || !familyCurrent(deps, source.family)) return false;
  if (deps.windows?.ownerOfContents(contents) !== owner || contents.session !== source.session) return false;
  if (deps.persona.policyEpoch !== source.epoch || deps.persona.partitionName() !== source.partition) return false;
  return tab.window ? tab.window.isFocused() : owner.tabs.activeTabId === tab.id;
}

/** Popup ids are not necessarily claimed by the opener's agent; preserve the entire native adoption provenance. */
function passkeyFamily(deps: PasskeyDeps, tab: Tab): Relative[] | null {
  const family: Tab[] = [tab];
  for (let child = tab; tab.window && child.openerId !== undefined;) {
    const parent = deps.windows?.owner(child.openerId)?.tabs.find(child.openerId);
    if (!parent || family.includes(parent)) return null;
    family.push(parent);
    child = parent;
  }
  return family.map((tab) => ({ tab, owner: deps.windows!.owner(tab.id)! }));
}

/** Closing, replacing, reparenting or claiming an opener invalidates a previously admitted popup. */
function familyCurrent(deps: PasskeyDeps, family: Relative[]): boolean {
  return family.every(
    ({ tab, owner }, index) =>
      deps.agentTab.humanMay(tab.id) &&
      deps.windows?.owner(tab.id) === owner &&
      owner.tabs.find(tab.id) === tab &&
      (!family[0].tab.window || tab.openerId === family[index + 1]?.tab.id),
  );
}

/** Preserve the requester and its ancestors so an unrelated iframe cannot cancel an account choice. */
function ancestors(frame: WebFrameMain): Set<WebFrameMain> {
  const frames = new Set<WebFrameMain>();
  for (let current: WebFrameMain | null = frame; current; current = current.parent) frames.add(current);
  return frames;
}

/** One bounded native prompt owns its listeners and cancellation signal. */
export class PasskeyRequest {
  /** Native cancellation signal passed only to Electron's message box. */
  readonly controller = new AbortController();
  /** Removes every listener and timer when the dialog settles. */
  private readonly cleanup: (() => void)[] = [];
  /** Mutable authorization services are consulted again throughout the native prompt. */
  private readonly deps: PasskeyDeps;
  /** Exact request snapshot stays bound to the original native document. */
  readonly source: PasskeySource;
  /** Source and ownership are rechecked without trusting page events or URL text. */
  constructor(deps: PasskeyDeps, source: PasskeySource) {
    this.deps = deps;
    this.source = source;
    this.watch();
  }
  /** All cancellation paths revoke the same request, including a late account response. */
  readonly cancel = (): void => this.controller.abort();
  /** Control and native lifetime observers share the same admission predicate. */
  readonly sync = (): void => {
    if (!this.current()) this.cancel();
  };
  /** A revoked request never becomes valid again when control/profile returns. */
  current(): boolean {
    return !this.controller.signal.aborted && passkeyCurrent(this.deps, this.source);
  }
  /** Capture exact native frame ancestry before any navigation replaces it. */
  private watch(): void {
    const frames = ancestors(this.source.frame);
    const navigate = (event: WebContentsDidStartNavigationEventParams): void => {
      if (!event.isSameDocument && (event.isMainFrame || !event.frame || frames.has(event.frame))) this.cancel();
    };
    this.listen(navigate);
    this.timers();
    const unsubscribe = this.deps.windows?.tabEvents.subscribe(this.sync);
    if (unsubscribe) this.cleanup.push(unsubscribe);
  }
  /** The native API has no frame-removal event; bounded checks catch detachment while a chooser is open. */
  private timers(): void {
    const timeout = setTimeout(this.cancel, PASSKEY_PROMPT_TIMEOUT_MS);
    const recheck = setInterval(this.sync, PASSKEY_RECHECK_MS);
    this.cleanup.push(() => {
      clearTimeout(timeout);
      clearInterval(recheck);
    });
  }
  /** Native navigation and parent destruction revoke consent promptly. */
  private listen(navigate: (event: WebContentsDidStartNavigationEventParams) => void): void {
    const contents = this.source.contents;
    contents.on('did-start-navigation', navigate);
    contents.once('destroyed', this.cancel);
    this.source.window.once('closed', this.cancel);
    this.cleanup.push(() => this.unlisten(navigate));
  }
  /** Remove observers even when Electron rejects the dialog promise. */
  private unlisten(navigate: (event: WebContentsDidStartNavigationEventParams) => void): void {
    const contents = this.source.contents;
    contents.removeListener('did-start-navigation', navigate);
    contents.removeListener('destroyed', this.cancel);
    this.source.window.removeListener('closed', this.cancel);
  }
  /** A finished request cannot retain a page, timer or native window. */
  dispose(): void {
    this.cancel();
    for (const cleanup of this.cleanup.splice(0)) cleanup();
  }
}
