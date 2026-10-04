/**
 * The address bar's navigation of the active tab: pull the host's cookies,
 * wait for the tab's protection, then load, unless the person typed another
 * address, closed the tab, or lost control in the meantime.
 */
import type { AppServices } from '../app/services.ts';
import { WEB_URL, SEARCH_URL } from './constants.ts';
import { loadInTab, isUnprotected, showUnprotected } from './load.ts';
import type { Tab } from './types.ts';

/** The services the address bar uses. */
type Deps = Pick<AppServices, 'tabs' | 'control' | 'recorder' | 'cookies'>;

/** A host, with an optional port and path: has a dot, or is localhost, and no spaces. */
const HOST = /^(localhost|[^\s/:]+\.[^\s/:]+)(:\d+)?(\/\S*)?$/i;
/** This machine or a bare IP, which usually serves plain http. */
const PLAIN_HTTP = /^(localhost|127\.|\d{1,3}(\.\d{1,3}){3}[:/]?|\[::1\])/i;

/**
 * Whether a tab may be sent here: an http(s) address or about:blank. file: reads
 * this machine, javascript: runs as code in the page, and both the person's
 * address bar and a caller's command go through this one rule.
 */
export function isWebAddress(url: unknown): boolean {
  const trimmed = String(url).trim();
  return WEB_URL.test(trimmed) || trimmed.toLowerCase() === 'about:blank';
}

/** What a command is told when it asks for any other address. */
export const NOT_A_WEB_ADDRESS = 'Only http and https addresses, or about:blank, can be opened.';

/**
 * What the address bar loads for what was typed, as any browser does: an http(s)
 * address as it is, a bare host over https (http for this machine or an IP),
 * about:blank, and anything else as a search. Other schemes (file:, javascript:)
 * are searched, never opened: the bar must not reach local files.
 */
export function normalizeAddress(url: unknown): string {
  const trimmed = String(url).trim();
  if (isWebAddress(trimmed)) return trimmed;
  if (HOST.test(trimmed)) return (PLAIN_HTTP.test(trimmed) ? 'http://' : 'https://') + trimmed;
  return SEARCH_URL + encodeURIComponent(trimmed);
}

/** Navigates the active tab to what the person typed. */
export class AddressBar {
  /** The tabs, who has control, the recording and the cookie pool. */
  private readonly deps: Deps;

  /** `deps` are the main-process services (see services.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Navigates the active tab to what the person typed. */
  async navigate(typed: string): Promise<void> {
    const view = this.deps.tabs.getActiveView();
    if (!view) return;
    const tab = this.deps.tabs.list.find((t: Tab) => t.view === view)!; // the active view is a listed tab's
    const request = this.begin(tab);
    const url = normalizeAddress(typed);
    // The recording collects the page's typing before the load below replaces the page.
    await Promise.all([this.deps.recorder.recordNavigation(url), this.deps.cookies.pullCookiesFor(url), tab.setup]);
    this.finish(tab, url, request);
  }

  /** Marks a tab as navigating; returns the request number that must still be current at the end. */
  private begin(tab: Tab): number {
    const request = (tab.navigationRequest = (tab.navigationRequest || 0) + 1);
    tab.navigationPending = true;
    tab.loadError = null;
    this.deps.tabs.sendTabList();
    return request;
  }

  /** Loads the page, unless the tab closed, another navigation took over, or control moved. */
  private finish(tab: Tab, url: string, request: number): void {
    const { tabs } = this.deps;
    if (tab.view.webContents.isDestroyed() || tab.navigationRequest !== request) return;
    tab.navigationPending = false;
    if (this.deps.control.snapshot().interactive)
      loadInTab(tab, url).catch((e) => isUnprotected(e) && showUnprotected(tabs, tab));
    tabs.sendTabList();
  }
}
