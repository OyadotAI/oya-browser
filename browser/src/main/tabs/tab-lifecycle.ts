/** Rendering and initial tab records, independent of the manager's selection and lifetime. */
import { mountOnly } from '../shell/view-stack.ts';
import type { BrowserView, Session, WebPreferences } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Tab, TabView } from './types.ts';
import { isHome, shownViewOf, staysHome, leaveHomeFor } from './home.ts';
import { sendZoom } from '../shell/page-zoom.ts';
import { recordingPreferences } from '../recording/preload.ts';
import { isNativeEngine } from '../native-policy/index.ts';

/**
 * A tab's page settings. Background throttling is off so a tab keeps drawing while
 * the window is hidden or covered, and an agent's click is not left waiting for a
 * frame (see KEEP_RENDERING_SWITCHES in src/main/app/constants.ts).
 */
export const tabPreferences = (partition: string, appDir: string, owner: Session): WebPreferences => ({
  ...recordingPreferences(appDir, !isNativeEngine(owner)),
  partition,
  backgroundThrottling: false,
});

/** A tab's page settings in a session the app already holds (an opener's), rather than a partition by name. */
export const sessionPreferences = (appDir: string, session: Session): WebPreferences => ({
  ...tabPreferences('', appDir, session),
  session,
});

/** Initial tab state never loads a URL; protection still owns the first navigation. */
export function initialTab(id: number, view: TabView, url: string): Tab {
  const home = isHome(url);
  return { id, view, title: home ? 'Oya' : 'New Tab', url: home ? '' : url || '', home };
}
/** What mounting a tab touches: the window, its layout, its strip, and the find bar that must not follow to another page. */
type MountDeps = Pick<AppServices, 'overlays' | 'shell' | 'layout' | 'tabs' | 'findBar'>;

/** Mount the owned view without recreating its renderer, cookies or navigation history. */
export function mountTab(deps: MountDeps, tab: Tab): void {
  const shown = shownViewOf(tab) as BrowserView | null;
  if (!deps.overlays.names.size) mountOnly(deps.shell.window!, shown);
  if (tab.home) deps.shell.window!.webContents.focus();
  deps.findBar.follow(tab.id);
  deps.layout.layoutActiveTab();
  announceTab(deps, tab);
}

/** Tells the toolbar and the strip about the tab now shown: its address, its zoom, its title. */
function announceTab(deps: MountDeps, tab: Tab): void {
  deps.shell.send('url-changed', tab.url);
  sendZoom(deps, false);
  deps.shell.send('title-changed', tab.title);
  deps.tabs.sendTabList();
}

/** Navigation metadata follows the owning window, including after a live transfer. */
export function updateTabUrl(tabs: AppServices['tabs'], shell: AppServices['shell'], tab: Tab, url: string): void {
  if (staysHome(tab, url)) return;
  tab.url = url;
  const shown = tab.id === tabs.activeTabId;
  if (leaveHomeFor(tab, url) && shown) return tabs.showInShell(tab);
  if (shown) shell.send('url-changed', url);
  // A new site has its own zoom, and it is already in effect when the address changes.
  if (shown) sendZoom({ tabs, shell }, false);
  tabs.sendTabList();
}
/** Titles update only their owning shell's selected address bar. */
export function updateTabTitle(tabs: AppServices['tabs'], shell: AppServices['shell'], tab: Tab, title: string): void {
  if (tab.home) return;
  tab.title = title;
  if (tab.id === tabs.activeTabId) shell.send('title-changed', title);
  tabs.sendTabList();
}
