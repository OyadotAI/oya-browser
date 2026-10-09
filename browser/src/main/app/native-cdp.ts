/** Compose the external native CDP front door from existing tab protection, command and ownership services. */
import {
  NativeInspection,
  NativeNavigationHistory,
  NativeRuntime,
  NativeFrameTree,
  NativeDeviceMetrics,
  watchNativeLog,
  watchNativePage,
  nativePageCommand,
  insertNativeText,
  type NativeEventSink,
} from '../native/index.ts';
import { AppNativeBrowser } from './native-browser.ts';
import type { NetworkDependencies } from '../native-network/index.ts';
import type { BrowserWindow } from 'electron';
import { randomUUID } from 'node:crypto';
import type { AppServices } from './services.ts';
import type { Tab } from '../tabs/types.ts';
import { PageDriver } from '../actions/driver.ts';
import { whenProtected } from '../tabs/load.ts';
import { isWebAddress, NOT_A_WEB_ADDRESS } from '../tabs/navigation.ts';
import {
  startNativeFrontDoor,
  nativeDoorConfig,
  type NativeBackend,
  type NativeTarget,
  type NativeSubscription,
} from '../native-front-door/index.ts';
import type { CommandParams } from '../actions/types.ts';

/** Native identities are scoped to live tab objects and never obtained from a debugging API. */
export class AppNativeBackend implements NativeBackend {
  /** Browser-owned tabs and control-plane dependencies. */
  private readonly deps: AppServices;
  /** Identity survives tab moves but cannot be reused by a replacement renderer/tab. */
  private readonly ids: WeakMap<Tab, string>;
  /** Document handles belong to one external connection, never to the shared app driver. */
  private readonly inspection = new NativeInspection();
  /** Snapshot capabilities are private to this connection and exact tab. */
  private readonly history = new NativeNavigationHistory();
  /** Native download state never touches the default-profile session. */
  private readonly browser: AppNativeBrowser;
  /** Native V8 handles and contexts are private to this external connection. */
  private readonly runtime = new NativeRuntime((frame) => this.frameTree.id(frame));
  /** Frame and document identities are local to the authenticated connection. */
  private readonly frameTree = new NativeFrameTree();
  /** Device overrides are restored when this external connection leaves. */
  private readonly metrics = new NativeDeviceMetrics();
  /** Capture services without changing their authorization or protection rules. */
  constructor(deps: AppServices, ids = new WeakMap<Tab, string>()) {
    this.deps = deps;
    const identify = (tab: Tab) => this.id(tab);
    const policy = networkPolicy(deps, () => this.tabs(), this.frameTree, identify);
    this.browser = new AppNativeBrowser(deps, policy);
    this.ids = ids;
  }
  /** A new socket shares tab identities but cannot access another socket’s DOM handles. */
  connection(): NativeBackend {
    return new AppNativeBackend(this.deps, this.ids);
  }
  /** Release only this socket’s native inspection resources. */
  dispose(): void {
    this.inspection.dispose();
    this.runtime.dispose();
    this.frameTree.dispose();
    this.metrics.dispose();
    void this.browser.contexts.dispose().catch(() => console.error('[native-context] cleanup failed'));
  }
  /** Discovery subscribes to native tab ownership, rechecking human control before each signal. */
  watchTargets(changed: () => void): () => void {
    return this.windows().tabEvents.subscribe(() => {
      if (nativeCanObserve(this.deps)) changed();
    });
  }
  /** Native selection uses stable tab ownership, not the current window's coincidental selection. */
  activate(target: string): void {
    const tab = this.tab(target),
      window = this.targetWindow(tab);
    this.deps.tabs.activateTab(tab.id);
    window.show();
    window.focus();
    tab.view.webContents.focus();
  }
  /** Window lookup preserves exact tab ownership, including an owned popup surface. */
  private targetWindow(tab: Tab): BrowserWindow {
    const owner = this.windows().owner(tab.id);
    if (!owner) throw Error('Native target has no owning window');
    const window = tab.window || owner.shell.window;
    if (!window || window.isDestroyed()) throw Error('Native target window is unavailable');
    return window;
  }
  /** Observation cannot follow focus, leak human-held activity, or bypass tab protection. */
  subscribe(target: string, domain: string, emit: NativeEventSink, params = {}): NativeSubscription {
    const tab = this.tab(target);
    if (domain === 'Fetch' || domain === 'OyaNetwork')
      return this.browser.network(tab.view.webContents).watch(tab.view.webContents, domain, emit, params);
    if (domain === 'Runtime') return this.runtime.watch(tab.view, target, () => this.observable(tab), emit);
    if (domain === 'Page') return watchNativePage(tab.view, () => this.observable(tab), emit);
    if (domain === 'Log') return watchNativeLog(tab.view, () => this.observable(tab), emit);
    throw Error('Unsupported native event domain');
  }
  /** Every event rechecks the current control owner and exact live tab. */
  private observable(tab: Tab): boolean {
    return nativeCanObserve(this.deps) && this.tabs().includes(tab);
  }
  /** Home/internal/protection-pending tabs cannot become remote page targets. */
  private tabs(): Tab[] {
    return this.windows()
      .allTabs()
      .filter((tab) => exposed(tab) && this.browser.contexts.visible(tab.view.webContents.session));
  }
  /** Missing window ownership is an explicit unavailable capability. */
  private windows(): NonNullable<AppServices['windows']> {
    if (!this.deps.windows) throw Error('Native window registry unavailable');
    return this.deps.windows;
  }
  /** Issue one opaque id for this exact tab object. */
  private id(tab: Tab): string {
    if (!this.ids.has(tab)) this.ids.set(tab, randomUUID());
    return this.ids.get(tab)!;
  }
  /** Return only currently exposed targets, excluding every shell and control overlay. */
  targets(): NativeTarget[] {
    return this.tabs().map((tab) => ({
      targetId: this.id(tab),
      type: 'page',
      browserContextId: this.browser.contexts.id(tab.view.webContents.session),
      title: tab.view.webContents.getTitle(),
      url: tab.view.webContents.getURL(),
    }));
  }
  /** Recheck tab authorization immediately before every native operation. */
  private tab(target: string): Tab {
    const tab = this.tabs().find((candidate) => this.id(candidate) === target);
    if (!tab) throw Error('Target is unavailable or unprotected');
    return tab;
  }
  /** Tab creation keeps the existing persona/egress/protection setup path. */
  async open(url: string, context?: string): Promise<string> {
    if (!isWebAddress(url)) throw Error(NOT_A_WEB_ADDRESS);
    const id = context
      ? this.deps.tabs.createTab(url, true, undefined, this.browser.contexts.get(context))
      : this.deps.tabs.openForAutomation(url);
    return this.readyOpened(this.opened(id));
  }
  /** Resolve the newly created tab through global window ownership. */
  private opened(id: number | null): Tab {
    const tab = this.windows()
      .allTabs()
      .find((candidate) => candidate.id === id);
    if (!tab) throw Error('Native tab creation failed');
    return tab;
  }
  /** Failed setup does not strand an inaccessible remote-created tab. */
  private async readyOpened(tab: Tab): Promise<string> {
    try {
      await whenProtected(tab);
      await this.deps.actions.waitForTabReady(tab);
      return this.id(this.tab(this.id(tab)));
    } catch (error) {
      this.windows().owner(tab.id)?.tabs.closeTab(tab.id, { keepOne: false });
      throw error;
    }
  }
  /** Browser context commands retain connection ownership and normal native setup. */
  async manage(action: string, params: Record<string, unknown>, emit: NativeEventSink): Promise<object> {
    return this.browser.manage(action, params, emit);
  }
  /** Only native paused continuations can bypass the serial command queue; ownership is rechecked. */
  resolveRequest(target: string, id: string, cancel: boolean, reason?: string): void {
    const contents = this.tab(target).view.webContents;
    this.browser.network(contents).resolve(contents, id, cancel, reason);
  }
  /** Close only the exact tab in its current owning window. */
  async close(target: string): Promise<void> {
    const tab = this.tab(target);
    this.windows().owner(tab.id)?.tabs.closeTab(tab.id, { keepOne: false });
  }
  /** A per-command driver is pinned to the requested target; UI focus never selects its destination. */
  async execute(target: string, action: string, params: Record<string, unknown>): Promise<unknown> {
    const tab = this.tab(target);
    await whenProtected(tab);
    this.tab(target);
    if (action === 'network:body')
      return this.browser.network(tab.view.webContents).body(tab.view.webContents, params.requestId as string);
    if (action.startsWith('runtime:'))
      return this.runtime.execute(tab.view, target, action.slice('runtime:'.length), params);
    return this.native(tab, target, action, params);
  }
  /** Route supported operations without retargeting after protection settles. */
  private native(tab: Tab, target: string, action: string, params: Record<string, unknown>): Promise<unknown> | object {
    if (action.startsWith('history:')) return this.historyCommand(tab, action, params);
    if (action === 'input:text') return insertNativeText(tab.view, params.text as string);
    if (action.startsWith('page:')) return nativePageCommand(tab.view, action, params);
    if (action === 'metrics:set') return this.metrics.set(tab.view, params);
    if (action === 'metrics:clear') return this.metrics.clear(tab.view);
    if (action === 'frameTree') return this.frameTree.snapshot(tab.view, target);
    if (action.startsWith('dom:')) return this.inspection.read(tab.view, action.slice('dom:'.length), params);
    return this.perform(tab, action, params);
  }
  /** History destinations retain the same explicit web and egress restrictions as navigation. */
  private historyCommand(tab: Tab, action: string, params: Record<string, unknown>): object {
    if (action === 'history:read') return this.history.read(tab.view);
    return this.history.navigate(tab.view, params, (url) => isWebAddress(url) && this.deps.governance.allowed(url));
  }
  /** Capture only this command’s result without replacing the global result sink. */
  private async perform(tab: Tab, action: string, params: Record<string, unknown>): Promise<unknown> {
    let result: unknown;
    const sendResult: PageDriver['deps']['sendResult'] = (_id, ok, data, error) => {
      if (!ok) throw Error(error || 'Native command failed');
      result = data;
    };
    await this.driver(tab, sendResult).runPageAction(randomUUID(), action, params as CommandParams, tab.view);
    return result;
  }
  /** Reuse production tools but replace every active-tab lookup and result sink with exact command ownership. */
  private driver(tab: Tab, sendResult: PageDriver['deps']['sendResult']): PageDriver {
    const current = this.deps.actions;
    return new PageDriver({
      ...current.deps,
      keyboard: current.keyboard,
      mouse: current.mouse,
      sendResult,
      ...pinnedTab(tab),
    });
  }
}
/** This explicit native-only endpoint cannot start in the legacy debugging/proxy mode. */
export function startAppNativeCdp(deps: AppServices): void {
  const config = nativeDoorConfig(process.env);
  if (!config) return;
  if (!deps.nativeBrowsing) throw Error('Native CDP requires the native browser mode; legacy proxy is not a fallback');
  const server = startNativeFrontDoor({ ...config, ...nativeServices(deps) });
  observeServer(server, deps);
}
/** Bind shutdown and diagnostic hooks without exposing credentials. */
function observeServer(server: ReturnType<typeof startNativeFrontDoor>, deps: AppServices): void {
  server.on('error', (error) => console.error('[native-cdp]', error.message));
  deps.electron.app.once('will-quit', () => server.close());
  server.on('listening', () => console.log('[native-cdp] authenticated loopback endpoint ready'));
}

/** Exclude internal documents and any tab whose protection is not complete. */
function exposed(tab: Tab): boolean {
  return (
    !tab.home &&
    tab.protection === 'protected' &&
    !tab.view.webContents.isDestroyed() &&
    isWebAddress(tab.view.webContents.getURL())
  );
}
/** Pin all current-tab lookups, including screenshots, to the commanded tab. */
function pinnedTab(tab: Tab): Pick<PageDriver['deps'], 'getActiveView' | 'tabs' | 'activeTabId'> {
  return { getActiveView: () => tab.view, tabs: () => [tab], activeTabId: () => tab.id };
}

/** Native dependencies keep both command admission and client ownership mandatory. */
function nativeServices(deps: AppServices) {
  return {
    backend: new AppNativeBackend(deps),
    beginCommand: () => deps.control.beginLocalCommand(),
    clientChanged: (delta: number) => deps.control.localClient(delta),
  };
}

/** Existing command ownership rules also constrain asynchronous browser activity. */
function nativeCanObserve(deps: AppServices): boolean {
  const c = deps.control;
  return !c.busy && !c.localHeld && (!c.connected || c.state.mode === 'agent');
}

/** Inputs keep the policy builder independent from private backend implementation details. */
type NetworkPolicyInputs = [
  deps: AppServices,
  tabs: () => Tab[],
  tree: NativeFrameTree,
  identify: (tab: Tab) => string,
];
/** Network attribution shares the authorized tab set and native frame identities, never URL guesses. */
function networkPolicy(...[deps, tabs, tree, identify]: NetworkPolicyInputs): NetworkDependencies {
  const target: NetworkDependencies['target'] = (wc) => identify(tabs().find((t) => t.view.webContents === wc)!);
  return {
    allowed: (wc) => nativeCanObserve(deps) && tabs().some((t) => t.view.webContents === wc),
    allowedURL: (url) => deps.governance.allowed(url),
    frameId: (wc, frame) => (frame === wc.mainFrame ? target(wc) : tree.id(frame)),
    target,
  };
}
