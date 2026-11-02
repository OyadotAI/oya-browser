/** Prepare complete window chrome before committing ownership of a live tab. */
import type { Point } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Tab } from '../tabs/types.ts';
import { HOME_URL } from '../tabs/constants.ts';
import type { Session } from 'electron';
import { discardWindow } from './cleanup.ts';
import type { BrowserWindows } from './windows.ts';
import { SIGN_IN_FIRST } from './constants.ts';
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
  /** Home also waits for its tab and address field before becoming visible; `session` opens an incognito window. */
  async newWindow(session?: Session): Promise<void> {
    const scope = this.windows.create(undefined, session);
    await this.prepared(scope, scope.tabs.createTab(HOME_URL, true));
    try {
      this.windows.present(scope);
    } catch (error) {
      discardWindow(scope);
      throw error;
    }
  }
  /**
   * An incognito window for a signed-in person. Sign-in is checked again after
   * setup, which a sign-out may outlast; then incognito closes and its session
   * retires, so no later window reuses one protected for an earlier persona.
   */
  async newIncognitoWindow(root: AppServices): Promise<void> {
    if (!root.shell.browsingMode) throw Error(SIGN_IN_FIRST);
    const session = await root.incognito.get();
    if (root.shell.browsingMode) return this.newWindow(session);
    this.windows.closeIncognito();
    throw Error(SIGN_IN_FIRST);
  }
  /** Serialize repeated drops for the same page; `fresh` always opens a new window. Answers where the tab went. */
  async detach(id: number, point?: Point, fresh = false): Promise<AppServices | undefined> {
    if (this.pending.has(id)) return;
    this.pending.add(id);
    try {
      return await this.move(id, point, fresh);
    } finally {
      this.pending.delete(id);
    }
  }
  /** Source stays mounted until a hidden shell is ready. */
  private async move(id: number, point: Point | undefined, fresh: boolean): Promise<AppServices | undefined> {
    const source = this.windows.owner(id);
    const tab = source?.tabs.find(id);
    if (!source || !tab || tab.window) return;
    this.windows.assertMovable(source, id);
    const existing = point && !fresh ? this.windows.dropTarget(source, point) : undefined;
    const target = existing ?? (await this.prepare(source, tab, point));
    this.commit(source, target, tab, !existing);
    return target;
  }
  /** A failed handoff removes only an uncommitted destination. */
  private commit(source: AppServices, target: AppServices, tab: Tab, fresh: boolean): void {
    try {
      this.windows.transfer(source, target, tab);
    } catch (error) {
      if (fresh && !target.tabs.find(tab.id)) discardWindow(target);
      throw error;
    }
  }
  /** Stage metadata only, not the live view. The new window is the same kind as the one the tab leaves. */
  private async prepare(source: AppServices, tab: Tab, point?: Point): Promise<AppServices> {
    const scope = this.windows.create(point, source.tabs.privateSession);
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
      discardWindow(scope);
      throw error;
    }
  }
}
