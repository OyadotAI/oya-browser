/**
 * The one way a tab loads a page: after its protection has settled, and never
 * in a tab that could not be protected. The first page, the address bar and a
 * navigate command all come through here, so none of them can load a page
 * with no fingerprint while setup is still pending or after it failed.
 */
import type { LoadURLOptions } from 'electron';
import { TAB_UNPROTECTED, TAB_UNPROTECTED_DESKTOP } from './constants.ts';
import type { Tab, TabListSink } from './types.ts';

/** The code a caller branches on, carried on the error to the server. */
const UNPROTECTED_CODE = 'tab_unprotected';

/** A tab as far as loading in it goes: its view, and its protection while it settles. */
export type LoadableTab = Pick<Tab, 'view' | 'setup' | 'protection'>;

/** Any tab, as far as its protection goes: an adopted popup or a front-door tab may have no setup at all. */
type ProtectableTab = object & Partial<Pick<Tab, 'setup' | 'protection'>>;

/** An error that carries a code a caller branches on. */
interface CodedError extends Error {
  /** What kind of failure it is. */
  code?: string;
}

/** The refusal for a tab that could not be protected. */
export function unprotected(): CodedError {
  const err: CodedError = new Error(TAB_UNPROTECTED);
  err.code = UNPROTECTED_CODE;
  return err;
}

/** Whether `err` is that refusal. */
export const isUnprotected = (err: unknown): boolean =>
  typeof err === 'object' && err !== null && 'code' in err && err.code === UNPROTECTED_CODE;

/**
 * Resolves once the tab's protection has settled; throws for a tab it failed
 * on. `tab.setup` always settles (two bounded attempts), and a tab the app
 * adopted from a popup has none, so it goes on as it always has.
 */
export async function whenProtected(tab: ProtectableTab | null | undefined): Promise<void> {
  await tab?.setup;
  if (tab?.protection === 'failed') throw unprotected();
}

/** Loads `url` once the tab's protection has settled; refuses a tab it failed on. */
export async function loadInTab(tab: LoadableTab, url: string, options?: LoadURLOptions): Promise<void> {
  await whenProtected(tab);
  return tab.view.webContents.loadURL(url, options);
}

/** Shows the person why the tab stays empty, and offers no Reload. */
export function showUnprotected(tabs: TabListSink, tab: Tab): void {
  tab.navigationPending = false;
  tab.loadError = TAB_UNPROTECTED_DESKTOP;
  tabs.sendTabList();
}
