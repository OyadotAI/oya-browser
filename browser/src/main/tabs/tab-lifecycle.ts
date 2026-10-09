/** Rendering and initial tab records, independent of the manager's selection and lifetime. */
import type { BrowserView } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Tab, TabView } from './types.ts';
import { isHome, shownViewOf, staysHome, leaveHomeFor } from './home.ts';

/** Initial tab state never loads a URL; protection still owns the first navigation. */
export function initialTab(id: number, view: TabView, url: string): Tab {
  const home = isHome(url);
  return { id, view, title: home ? 'Oya' : 'New Tab', url: home ? '' : url || '', home };
}
/** Mount the owned view without recreating its renderer, cookies or navigation history. */
export function mountTab(deps: Pick<AppServices, 'overlays' | 'shell' | 'layout' | 'tabs'>, tab: Tab): void {
  const shown = shownViewOf(tab) as BrowserView | null;
  if (!deps.overlays.names.size) deps.shell.window!.setBrowserView(shown);
  if (tab.home) deps.shell.window!.webContents.focus();
  deps.layout.layoutActiveTab();
  deps.shell.send('url-changed', tab.url);
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
  tabs.sendTabList();
}
/** Titles update only their owning shell's selected address bar. */
export function updateTabTitle(tabs: AppServices['tabs'], shell: AppServices['shell'], tab: Tab, title: string): void {
  if (tab.home) return;
  tab.title = title;
  if (tab.id === tabs.activeTabId) shell.send('title-changed', title);
  tabs.sendTabList();
}
