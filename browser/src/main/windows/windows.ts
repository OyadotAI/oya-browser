/** Native browser windows own independent shells while sharing the profile and globally unique tab ids. */
import type { Event, Point, Session, WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Tab } from '../tabs/types.ts';
import { closeWindowTabs, discardWindow, disposeWindow } from './cleanup.ts';
import { assertSameKind, isPrivateScope, publicScopes, sameKind } from './kinds.ts';
import { inTabStrip, inWindow, placeNear } from './geometry.ts';
import { TabTearOff } from './tear-off.ts';
import { WindowOpening } from './opening.ts';
import { WindowContext } from './context.ts';
import { WindowServiceRouter } from './service-router.ts';
import { WindowTabEvents } from './tab-events.ts';

/** One application-level owner for window lifetimes, transfer and agent tab discovery. */
export class BrowserWindows {
  /** Browser-process change signals for authorized native observers. */
  readonly tabEvents = new WindowTabEvents();
  /** Shared application services; never replaced by a per-window dependency object. */
  private readonly root: AppServices;
  /** A tab pulled off a strip, carried in its own window under the pointer. */
  readonly drag: TabTearOff = new TabTearOff(this);
  /** Live dependency scopes, including the primary before its native window is created. */
  private readonly scopes: AppServices[] = [];
  /** Last selected window; remains available during final application shutdown. */
  current: AppServices;
  /** Monotonic application-wide id allocation prevents collisions after transfer or close. */
  private nextId = 1;
  /** Set once a quit saved the session: the windows close one by one, and a later save would keep only the last one's tabs. */
  private quitSaved = false;
  /** Hidden preparation and transactional transfer lifetime. */
  private readonly opening: WindowOpening;
  /** Build the first local scope, then expose stable application-level routers. */
  constructor(root: AppServices) {
    this.root = root;
    this.watchQuit();
    this.current = this.addScope();
    this.opening = new WindowOpening(this);
    new WindowServiceRouter(root, this).install();
  }
  /** An update restart closes every window before 'before-quit' fires; it commits the quit first. Lifecycle commits every other quit. */
  private watchQuit(): void {
    this.root.electron.autoUpdater.on('before-quit-for-update', () => this.root.callGuard.commit());
  }
  /** A fresh globally unique tab id. */
  allocateTabId(): number {
    return this.nextId++;
  }
  /** All tabs, including real OAuth popups, remain visible to the agent; incognito windows' never do. */
  allTabs(): Tab[] {
    return publicScopes(this.scopes).flatMap((scope) => scope.tabs.list);
  }
  /** Invoke a shared notification against a snapshot, safe when a callback closes a window. */
  each(run: (scope: AppServices) => unknown): void {
    [...this.scopes].forEach(run);
  }
  /** Find the owning scope from the stable tab id, not URL or web page state. */
  owner(id: number): AppServices | undefined {
    return this.scopes.find((scope) => scope.tabs.find(id));
  }
  /** The owner an application-level call by id may reach: an incognito tab's id reads as not found. */
  publicOwner(id: number): AppServices | undefined {
    return publicScopes(this.scopes).find((scope) => scope.tabs.find(id));
  }
  /** Tab input and native permissions resolve moved page ownership without page-provided ids. */
  ownerOfContents(contents: WebContents): AppServices | undefined {
    return this.scopes.find((scope) => scope.tabs.list.some((tab) => tab.view.webContents === contents));
  }
  /** Only registered shell webContents may access shell IPC. */
  fromContents(contents: WebContents): AppServices | undefined {
    return this.scopes.find((scope) => scope.shell.window?.webContents === contents && scope.shell.alive());
  }
  /** Select an application target without remounting any other window's selected page; never an incognito window. */
  select(scope: AppServices): void {
    if (this.scopes.includes(scope) && !isPrivateScope(scope)) this.current = scope;
  }
  /** A tab's callbacks resolve its current owner even after its view moves to another window. */
  forTab(tab: Tab, fallback: AppServices): AppServices {
    return new Proxy(fallback, { get: (_scope, key) => Reflect.get(this.owner(tab.id) ?? fallback, key) });
  }
  /** ShellWindow calls this once after creating a native window. */
  watch(scope: AppServices): void {
    const win = scope.shell.window!;
    win.on('focus', () => this.focused(scope));
    win.on('close', (event) => this.beforeClose(scope, event));
    win.on('closed', () => this.closed(scope));
  }
  /** Track native stacking separately from an agent selecting a background target. */
  private focused(scope: AppServices): void {
    const index = this.scopes.indexOf(scope);
    if (index < 0) return;
    this.scopes.splice(index, 1);
    this.scopes.push(scope);
    if (this.root.control.snapshot().interactive) this.select(scope);
  }
  /** A native close button cannot destroy an agent's page while its task is in flight, nor end a call without asking. */
  private beforeClose(scope: AppServices, event: Event): void {
    const { callGuard } = this.root;
    if (!callGuard.committed && scope.tabs.list.some((tab) => !this.root.agentTab.humanMay(tab.id)))
      return event.preventDefault();
    if (callGuard.holdClose(scope.shell.window!, scope.tabs.list, event)) return;
    this.saveSession();
    closeWindowTabs(scope);
  }
  /**
   * Saves the session before a window's tabs close. The red button closes a
   * window before 'before-quit' fires, so it saves each time; a quit (or an
   * update restart, whose windows close first) saves once, while every
   * window's tabs are still open.
   */
  private saveSession(): void {
    const quitting = this.root.callGuard.committed;
    if (quitting && this.quitSaved) return;
    this.quitSaved = quitting;
    this.root.sessionTabs?.flush();
  }
  /** Open a full browser shell; never create a second profile or duplicate the server connection. `session` makes it incognito. */
  create(point?: Point, session?: Session): AppServices {
    this.root.agentTab.requireFor(null);
    const scope = this.addScope(session);
    scope.shell.browsingMode = true;
    scope.shell.create(true);
    if (point) placeNear(scope, point);
    return scope;
  }
  /** Keyboard/menu new-window starts on Home, whose address box receives focus. */
  newWindow(): Promise<void> {
    return this.opening.newWindow();
  }
  /** A window in the shared incognito session; only for a signed-in person, never governed or headless. */
  newIncognitoWindow(): Promise<void> {
    return this.opening.newIncognitoWindow(this.root);
  }
  /** Shuts every incognito window (persona switch, sign out, last normal window) and retires the session, even one still being set up, so the next window never reuses one protected for an earlier persona. */
  closeIncognito(): void {
    this.scopes.filter(isPrivateScope).forEach(discardWindow);
    void this.root.incognito.retire();
  }
  /** Move a live tab only after the destination chrome is drawn. */
  async detach(id: number, point?: Point): Promise<void> {
    await this.opening.detach(id, point);
  }
  /** Moves a live tab into a new window at `point`, whatever is under it; answers that window. */
  tearOff(id: number, point: Point): Promise<AppServices | undefined> {
    return this.opening.detach(id, point, true);
  }
  /** Reveal an already-painted window as one complete surface. */
  present(scope: AppServices): void {
    this.root.agentTab.requireFor(null);
    this.select(scope);
    scope.shell.window?.show();
    scope.shell.window?.focus();
  }
  /** Overlays retain captured page state; moving their underlying view would violate that ownership. */
  assertMovable(source: AppServices, id: number): void {
    this.root.agentTab.requireFor(id);
    if (source.overlays.names.size) throw new Error('Close the open menu or dialog before moving this tab');
  }
  /**
   * The window whose tab strip is under `point`: the frontmost other window there,
   * so a drop never falls through it into a strip it covers. `source` is skipped,
   * since a torn-off tab's own window sits under the pointer that carries it. Only
   * a window of the same kind takes the tab: a normal tab never joins an incognito
   * window, nor the other way round.
   */
  dropTarget(source: AppServices, point: Point): AppServices | undefined {
    const hit = this.frontmostAt(point, source);
    if (!hit || !sameKind(hit, source)) return undefined;
    return inTabStrip(point, hit.shell.window!.getContentBounds()) ? hit : undefined;
  }
  /** The frontmost open window under `point`, other than `skip`. */
  private frontmostAt(point: Point, skip: AppServices): AppServices | undefined {
    const open = (scope: AppServices) => scope.shell.alive() && scope.shell.window.isVisible();
    return [...this.scopes]
      .reverse()
      .find((scope) => scope !== skip && open(scope) && inWindow(point, scope.shell.window!.getBounds()));
  }
  /**
   * The original tab object, webContents, history and renderer survive the
   * transfer. The tab arrives unpinned: the target appends it, which is the
   * unpinned end, so the pinned block stays leading in both windows.
   */
  transfer(source: AppServices, target: AppServices, tab: Tab): void {
    this.assertMovable(source, tab.id);
    this.assertMovable(target, tab.id);
    this.assertSource(source, target, tab);
    source.tabs.releaseTab(tab.id);
    target.tabs.receiveTab(Object.assign(tab, { pinned: false }));
    target.shell.stagedTab = undefined;
    this.present(target);
    if (!source.tabs.list.length) source.shell.window?.close();
  }
  /** Recheck ownership after asynchronous shell preparation, and that the tab stays in its own kind of window. */
  private assertSource(source: AppServices, target: AppServices, tab: Tab): void {
    if (!source.shell.alive() || source.tabs.find(tab.id) !== tab)
      throw new Error('The source tab changed during the move');
    assertSameKind(source, target);
  }
  /** Register a dependency scope before creating tabs or loading any renderer. */
  private addScope(session?: Session): AppServices {
    const scope = new WindowContext(this.root, session).services;
    this.scopes.push(scope);
    return scope;
  }
  /** Closing one shell tears down only its tabs, shield and layout timers, not the shared session. */
  private closed(scope: AppServices): void {
    disposeWindow(scope);
    this.scopes.splice(this.scopes.indexOf(scope), 1);
    const normal = publicScopes(this.scopes);
    if (this.current === scope && normal.length) this.current = normal.at(-1)!;
    if (!isPrivateScope(scope) && !normal.length) this.closeIncognito();
    else if (isPrivateScope(scope) && !this.scopes.some(isPrivateScope)) void this.root.incognito.retire();
  }
}
