/** Prepare complete window chrome before committing ownership of a live tab. */
import type { Point } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Tab } from '../tabs/types.ts';
import { HOME_URL } from '../tabs/constants.ts';
import { closeWindowTabs } from './cleanup.ts';
import type { BrowserWindows } from './windows.ts';
/** Owns pending transfers so duplicate drops cannot duplicate a page. */
export class WindowOpening {
  /** Registry owns native windows and tab ownership. */
  private readonly windows: BrowserWindows;
  /** One pending operation per tab. */
  private readonly pending = new Set<number>();
  /** Uses the existing application registry. */
  constructor(windows: BrowserWindows) {
    this.windows = windows;
  }
  /** Home also waits for its tab and address field before becoming visible. */
  async newWindow(): Promise<void> {
    const scope = this.windows.create();
    await this.prepared(scope, scope.tabs.createTab(HOME_URL, true));
    try {
      this.windows.present(scope);
    } catch (error) {
      this.discard(scope);
      throw error;
    }
  }
  /** Serialize repeated drops for the same page. */
  async detach(id: number, point?: Point): Promise<void> {
    if (this.pending.has(id)) return;
    this.pending.add(id);
    try {
      await this.move(id, point);
    } finally {
      this.pending.delete(id);
    }
  }
  /** Source stays mounted until a hidden shell is ready. */
  private async move(id: number, point?: Point): Promise<void> {
    const source = this.windows.owner(id);
    const tab = source?.tabs.find(id);
    if (!source || !tab || tab.window) return;
    this.windows.assertMovable(source);
    const existing = point ? this.windows.dropTarget(source, point) : undefined;
    const target = existing ?? (await this.prepare(tab, point));
    this.commit(source, target, tab, !existing);
  }
  /** A failed handoff removes only an uncommitted destination. */
  private commit(source: AppServices, target: AppServices, tab: Tab, fresh: boolean): void {
    try {
      this.windows.transfer(source, target, tab);
    } catch (error) {
      if (fresh && !target.tabs.find(tab.id)) this.discard(target);
      throw error;
    }
  }
  /** Stage metadata only, not the live view. */
  private async prepare(tab: Tab, point?: Point): Promise<AppServices> {
    const scope = this.windows.create(point);
    scope.shell.stagedTab = tab;
    scope.tabs.sendTabList();
    await this.prepared(scope, tab.id);
    return scope;
  }
  /** Preparation failure leaves the original page untouched. */
  private async prepared(scope: AppServices, id: number): Promise<void> {
    try {
      await scope.shell.painted(id);
    } catch (error) {
      this.discard(scope);
      throw error;
    }
  }
  /** Detach owned views before destroying a failed hidden shell. */
  private discard(scope: AppServices): void {
    closeWindowTabs(scope);
    scope.shell.window?.destroy();
  }
}
