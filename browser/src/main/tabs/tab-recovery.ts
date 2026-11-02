/**
 * A person's Reload on a page whose renderer is stuck. Reloading a hung page lands
 * in the same stuck process and stays on "Loading…", so a person clicked Reload
 * again and again. Chromium's own hang signal is off in this app (--disable-hang-monitor,
 * anonymity/telemetry.ts), so Reload asks the page itself first: no answer means
 * its renderer is ended, and the page is reloaded
 * once Electron reports that renderer gone (reloading straight after the kill
 * never loads). Only a person's click comes here; an agent's reload never does.
 */
import type { WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';
import { ANALYZER_WORLD_ID, runInWorld } from '../native/index.ts';
import { isNativeEngine } from '../native-policy/index.ts';
import { TAB_PROBE_MS } from './constants.ts';
import type { Tab } from './types.ts';

/** What recovery reads: who may act on the tab, the tabs, and the health log when there is one. */
export type RecoveryDeps = Pick<AppServices, 'agentTab' | 'tabs'> & Partial<Pick<AppServices, 'tabHealthLog'>>;

/** Puts a message on the tab (TabEvents.showTabError). */
type ShowError = (message: string) => void;

/**
 * Whether the page's renderer runs code within `wait` ms. It asks the tab preload
 * in an isolated world (the page never sees it), which answers at once even while
 * the page loads, so a slow load is not a hang; it never falls back to the stock API,
 * which waits for the load. A refusal means a new document took the frame: alive.
 * Only silence means stuck. The patched engine's tabs have no preload that answers,
 * so they are never asked, and Reload there works as it always has.
 */
export function alive(contents: WebContents, wait = TAB_PROBE_MS): Promise<boolean> {
  if (isNativeEngine(contents.session)) return Promise.resolve(true);
  return new Promise((resolve) => {
    const timer = setTimeout(() => resolve(false), wait);
    const answered = (): void => (clearTimeout(timer), resolve(true));
    runInWorld(contents, ANALYZER_WORLD_ID, '1', true).then(answered, answered);
  });
}

/**
 * A person's Reload of `tab`: a page that does not answer has its renderer ended
 * (the reload follows in rendererGone); one that answers gets `otherwise` (reload, or
 * stop a load). A second click while one is being checked or recovered does nothing.
 */
export async function reloadOrRecover(deps: RecoveryDeps, tab: Tab, otherwise: () => void): Promise<void> {
  if (tab.probing || tab.recovering) return;
  if (tab.window || tab.home || tab.view.webContents.isCrashed()) return otherwise();
  tab.probing = true;
  try {
    await askThenAct(deps, tab, otherwise);
  } finally {
    tab.probing = false;
  }
}

/** Asks the page, then ends its stuck renderer or runs `otherwise`, if the tab is still the person's. */
async function askThenAct(deps: RecoveryDeps, tab: Tab, otherwise: () => void): Promise<void> {
  const answers = await alive(tab.view.webContents);
  if (!stillThePersons(deps, tab)) return;
  // A renderer that already died never answers, and ending it again is never reported: just reload.
  if (answers || tab.view.webContents.isCrashed()) otherwise();
  else endRenderer(deps, tab);
}

/** Whether the tab is still open, live, and the person's after the wait: an agent may have claimed it meanwhile. */
function stillThePersons(deps: RecoveryDeps, tab: Tab): boolean {
  const open = deps.tabs.list.includes(tab) && !tab.view.webContents.isDestroyed();
  return open && deps.agentTab.humanMay(tab.id);
}

/** Ends the stuck renderer. Every tab sharing it ends too, which is why only a person's click gets here. */
function endRenderer(deps: RecoveryDeps, tab: Tab): void {
  deps.tabHealthLog?.record(tab, 'probe-failed');
  tab.recovering = true;
  tab.view.webContents.forcefullyCrashRenderer();
}

/**
 * A renderer went away. The tab the person is recovering reloads now, in a fresh
 * renderer, with no crash message; any other tab says it stopped. Electron always
 * reports a renderer it was told to end, so `recovering` never outlives that report.
 */
export function rendererGone(deps: RecoveryDeps, tab: Tab, reason: string, show: ShowError): void {
  deps.tabHealthLog?.record(tab, 'gone', reason);
  if (tab.recovering) restart(deps, tab);
  else show(`Page renderer stopped (${reason}). Reload to recover.`);
}

/** Reloads the recovered tab in a fresh renderer, clean of the hang and its message. */
function restart(deps: RecoveryDeps, tab: Tab): void {
  tab.recovering = false;
  tab.loadError = null;
  tab.view.webContents.reload();
  deps.tabs.sendTabList();
}
