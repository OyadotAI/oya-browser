/** Application-level callers follow the selected window; shell IPC keeps a fixed per-window scope. */
import type { AppServices } from '../app/services.ts';
import type { BrowserWindows } from './windows.ts';

/** Services that have a separate lifetime in each window. */
type LocalService = 'shell' | 'shortcuts' | 'shield' | 'layout' | 'overlays' | 'tabs';
/** Tab commands carrying an id must route to its owner, not whichever window is focused. */
const TAB_ID_METHODS = new Set(['activateTab', 'closeTab', 'find']);

/** Stable application facade, so a collaborator may safely retain a service reference. */
export class WindowServiceRouter {
  /** Window ownership and active application target. */
  private readonly windows: BrowserWindows;
  /** The graph whose collaborators receive stable routers. */
  private readonly root: AppServices;
  /** Install routers only after the primary local scope exists. */
  constructor(root: AppServices, windows: BrowserWindows) {
    this.root = root;
    this.windows = windows;
  }
  /** Root services are routers; each window retains concrete local service instances. */
  install(): void {
    const names: LocalService[] = ['shell', 'shortcuts', 'shield', 'layout', 'overlays', 'tabs'];
    for (const name of names) this.assign(name);
  }
  /** Route each property read and method at call time without changing a window-local service. */
  private assign(name: LocalService): void {
    const target = this.windows.current[name];
    const proxy = new Proxy(target, { get: (_target, key) => this.read(name, key) });
    Reflect.set(this.root, name, proxy);
  }
  /** Special aggregate operations are explicit; all other operations belong to the current window. */
  private read(name: LocalService, key: string | symbol): unknown {
    if (name === 'tabs' && key === 'list') return this.windows.allTabs();
    const value = Reflect.get(this.windows.current[name], key);
    if (typeof value !== 'function') return value;
    return (...args: unknown[]) => this.call(name, key, args);
  }
  /** Global broadcasts, global tab ids and local methods have different routing contracts. */
  private call(name: LocalService, key: string | symbol, args: unknown[]): unknown {
    if (name === 'tabs' && TAB_ID_METHODS.has(String(key))) return this.tabCall(String(key), args);
    if (this.broadcast(name, key)) return this.windows.each((scope) => this.invoke(scope, name, key, args));
    return this.invoke(this.windows.current, name, key, args);
  }
  /** Shared events reach every renderer and every independent control shield. */
  private broadcast(name: LocalService, key: string | symbol): boolean {
    return (
      (name === 'shell' && key === 'send') ||
      (name === 'shield' && key === 'controlChanged') ||
      (name === 'layout' && key === 'flush') ||
      (name === 'tabs' && ['sendTabList', 'leaveBrowsingMode'].includes(String(key)))
    );
  }
  /** Selecting an agent target switches the application target, while local shell selection stays local. */
  private tabCall(key: string, args: unknown[]): unknown {
    const scope = this.windows.owner(Number(args[0]));
    if (!scope) return undefined;
    if (key === 'activateTab') this.windows.select(scope);
    return this.invoke(scope, 'tabs', key, args);
  }
  /** Reflect.apply preserves the concrete service receiver and its private state. */
  private invoke(scope: AppServices, name: LocalService, key: string | symbol, args: unknown[]): unknown {
    const service = scope[name];
    return Reflect.apply(Reflect.get(service, key), service, args);
  }
}
