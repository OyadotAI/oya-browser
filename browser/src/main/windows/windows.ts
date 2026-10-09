/** Native browser windows own independent shells while sharing the profile and globally unique tab ids. */
import type { Event, Point, WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Tab } from '../tabs/types.ts';
import { closeWindowTabs, disposeWindow } from './cleanup.ts';
import { inTabStrip, inWindow } from './geometry.ts';
import { TabDragPreview } from './drag-preview.ts';
import { WindowOpening } from './opening.ts';
import { WindowContext } from './context.ts';
import { WindowServiceRouter } from './service-router.ts';
import { WindowTabEvents } from './tab-events.ts';
import { WINDOW_DROP_OFFSET } from './constants.ts';

/** One application-level owner for window lifetimes, transfer and agent tab discovery. */
export class BrowserWindows {
  /** Browser-process change signals for authorized native observers. */
  readonly tabEvents = new WindowTabEvents();
  /** Shared application services; never replaced by a per-window dependency object. */
  private readonly root: AppServices;
  /** Native visual feedback shared by all shell windows. */
  readonly drag = new TabDragPreview();
  /** Live dependency scopes, including the primary before its native window is created. */
  private readonly scopes: AppServices[] = [];
  /** Last selected window; remains available during final application shutdown. */
  current: AppServices;
  /** Monotonic application-wide id allocation prevents collisions after transfer or close. */
  private nextId = 1;
  /** An application quit may close windows even while automation owns the session. */
  private quitting = false;
  /** Hidden preparation and transactional transfer lifetime. */
  private readonly opening: WindowOpening;
  /** Build the first local scope, then expose stable application-level routers. */
  constructor(root: AppServices) {
    this.root = root;
    root.electron.app.on('before-quit', () => {
      this.quitting = true;
    });
    this.current = this.addScope();
    this.opening = new WindowOpening(this);
    new WindowServiceRouter(root, this).install();
  }
  /** A fresh globally unique tab id. */
  allocateTabId(): number {
    return this.nextId++;
  }
  /** All tabs, including real OAuth popups, remain visible to the agent. */
  allTabs(): Tab[] {
    return this.scopes.flatMap((scope) => scope.tabs.list);
  }
  /** Invoke a shared notification against a snapshot, safe when a callback closes a window. */
  each(run: (scope: AppServices) => unknown): void {
    [...this.scopes].forEach(run);
  }
  /** Find the owning scope from the stable tab id, not URL or web page state. */
  owner(id: number): AppServices | undefined {
    return this.scopes.find((scope) => scope.tabs.find(id));
  }
  /** Tab input and native permissions resolve moved page ownership without page-provided ids. */
  ownerOfContents(contents: WebContents): AppServices | undefined {
    return this.scopes.find((scope) => scope.tabs.list.some((tab) => tab.view.webContents === contents));
  }
  /** Only registered shell webContents may access shell IPC. */
  fromContents(contents: WebContents): AppServices | undefined {
    return this.scopes.find((scope) => scope.shell.window?.webContents === contents && scope.shell.alive());
  }
  /** Select an application target without remounting any other window's selected page. */
  select(scope: AppServices): void {
    if (this.scopes.includes(scope)) this.current = scope;
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
  /** A native close button cannot destroy an agent's page while its task is in flight. */
  private beforeClose(scope: AppServices, event: Event): void {
    if (!this.quitting && !this.root.control.snapshot().interactive) return event.preventDefault();
    closeWindowTabs(scope);
  }
  /** Open a full browser shell; never create a second profile or duplicate the server connection. */
  create(point?: Point): AppServices {
    this.root.shield.requireHumanControl();
    const scope = this.addScope();
    scope.shell.browsingMode = true;
    scope.shell.create(true);
    if (point) this.position(scope, point);
    return scope;
  }
  /** Keyboard/menu new-window starts on Home, whose address box receives focus. */
  newWindow(): Promise<void> {
    return this.opening.newWindow();
  }
  /** Move a live tab only after the destination chrome is drawn. */
  detach(id: number, point?: Point): Promise<void> {
    return this.opening.detach(id, point);
  }
  /** Reveal an already-painted window as one complete surface. */
  present(scope: AppServices): void {
    this.root.shield.requireHumanControl();
    this.select(scope);
    scope.shell.window?.show();
    scope.shell.window?.focus();
  }
  /** Overlays retain captured page state; moving their underlying view would violate that ownership. */
  assertMovable(source: AppServices): void {
    this.root.shield.requireHumanControl();
    if (source.overlays.names.size) throw new Error('Close the open menu or dialog before moving this tab');
  }
  /** A drop over another shell's tab strip joins that window, otherwise it creates a new one. */
  dropTarget(source: AppServices, point: Point): AppServices | undefined {
    const hit = [...this.scopes]
      .reverse()
      .find(
        (scope) =>
          scope.shell.alive() && scope.shell.window.isVisible() && inWindow(point, scope.shell.window.getBounds()),
      );
    if (!hit || hit === source) return undefined;
    return inTabStrip(point, hit.shell.window!.getContentBounds()) ? hit : undefined;
  }
  /** The original tab object, webContents, history and renderer survive the transfer. */
  transfer(source: AppServices, target: AppServices, tab: Tab): void {
    this.assertMovable(source);
    this.assertMovable(target);
    this.assertSource(source, tab);
    source.tabs.releaseTab(tab.id);
    target.tabs.receiveTab(tab);
    target.shell.stagedTab = undefined;
    this.present(target);
    if (!source.tabs.list.length) source.shell.window?.close();
  }
  /** Recheck ownership after asynchronous shell preparation. */
  private assertSource(source: AppServices, tab: Tab): void {
    if (!source.shell.alive() || source.tabs.find(tab.id) !== tab)
      throw new Error('The source tab changed during the move');
  }
  /** Keep a new window within the target display's work area, including negative-coordinate monitors. */
  private position(scope: AppServices, point: Point): void {
    const area = this.root.electron.screen.getDisplayNearestPoint(point).workArea;
    const bounds = scope.shell.window!.getBounds();
    const x = Math.max(area.x, Math.min(point.x - WINDOW_DROP_OFFSET, area.x + area.width - bounds.width));
    const y = Math.max(area.y, Math.min(point.y - WINDOW_DROP_OFFSET, area.y + area.height - bounds.height));
    scope.shell.window!.setPosition(Math.round(x), Math.round(y));
  }
  /** Register a dependency scope before creating tabs or loading any renderer. */
  private addScope(): AppServices {
    const scope = new WindowContext(this.root).services;
    this.scopes.push(scope);
    return scope;
  }
  /** Closing one shell tears down only its tabs, shield and layout timers, not the shared session. */
  private closed(scope: AppServices): void {
    disposeWindow(scope);
    this.scopes.splice(this.scopes.indexOf(scope), 1);
    if (this.current === scope && this.scopes.length) this.current = this.scopes.at(-1)!;
  }
}
