/**
 * A new tab's protection before its first page, as named stages: load
 * about:blank (a view has no renderer before its first navigation, and CDP's
 * Page domain does not answer before there is one), set up CDP within a time
 * limit, retry once after a reset, and fail closed if that fails too.
 */
import type { AppServices } from '../app/services.ts';
import { withinTime } from '../../shared/within-time.ts';
import { showUnprotected } from './load.ts';
import { CDP_SETUP_TIMEOUT } from './constants.ts';
import type { Tab, TabView } from './types.ts';

/** The services protecting a new tab uses. */
type Deps = Pick<AppServices, 'protection' | 'tabs'>;

/** The tab is protected: pages may load in it. */
function markProtected(tab: Tab): void {
  tab.protection = 'protected';
}

/** Protects each new tab before anything loads in it. */
export class TabProtector {
  /** Protection itself, and the tabs to tell when a tab could not be protected. */
  private readonly deps: Deps;

  /** `deps` are the main-process services (see services.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /**
   * Attach CDP debugger and auto-inject scripts into every new document, then
   * settle `tab.protection`. A view has no renderer until its first navigation,
   * and CDP's Page domain does not answer before there is one, so setup runs
   * after about:blank, which starts the renderer without a network request.
   * An attempt that fails or runs out of time is reset and tried once more; a
   * second failure leaves the tab on about:blank, closed to the web, since a
   * page loaded with no fingerprint shows the site this machine for good.
   * Always settles, within two attempts.
   */
  async protect(tab: Tab): Promise<void> {
    tab.protection = 'pending';
    const blank = tab.view.webContents.loadURL('about:blank').catch(() => {});
    if (await this.protectOnce(tab.view, blank)) return markProtected(tab);
    this.deps.protection.resetTabCDP(tab.view);
    console.error('[anonymity] tab protection did not finish on the first attempt, trying once more');
    if (await this.protectOnce(tab.view, blank)) return markProtected(tab);
    this.failClosed(tab);
  }

  /** One bounded attempt: the blank page, then setup. True only when setup says the tab is protected. */
  private protectOnce(view: TabView, blank: Promise<unknown>): Promise<boolean> {
    return withinTime(
      blank.then(() => this.deps.protection.setupTabCDP(view)),
      CDP_SETUP_TIMEOUT,
    ).then(
      (ok: unknown) => ok === true,
      () => false,
    );
  }

  /** Both attempts failed: nothing is loaded in this tab, and the person is told why. */
  private failClosed(tab: Tab): void {
    this.deps.protection.resetTabCDP(tab.view);
    tab.protection = 'failed';
    console.error('[anonymity] tab protection failed twice, tab not loaded');
    showUnprotected(this.deps.tabs, tab);
  }
}
