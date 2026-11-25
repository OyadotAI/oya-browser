/**
 * A tab opened only to download a file (a link with target="_blank", a typed
 * file address) is left blank once the download takes over. Chrome closes it;
 * so does Oya, but only a tab that never showed a page.
 */
import type { WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';

/** Addresses of a tab that never showed a page. */
const BLANK = new Set(['', 'about:blank']);

/** Closes the tab whose page is `contents` when it never showed a page. */
export function closeDownloadOnlyTab(deps: Pick<AppServices, 'tabs'>, contents: WebContents | undefined): void {
  const tab = deps.tabs.list.find((t) => t.view.webContents === contents);
  if (!tab || tab.home || !contents || contents.isDestroyed()) return;
  if (BLANK.has(contents.getURL()) && !contents.navigationHistory.canGoBack()) deps.tabs.closeTab(tab.id);
}

/** Whether the person may use the tab whose page is `contents`; a page that is no tab (a popup window) counts as new. */
export function personMayUse(
  deps: Pick<AppServices, 'tabs' | 'agentTab'> & Partial<Pick<AppServices, 'windows'>>,
  contents: WebContents | undefined,
): boolean {
  const scope = (contents && deps.windows?.ownerOfContents(contents)?.tabs) ?? deps.tabs;
  const tab = scope.list.find((t) => t.view.webContents === contents);
  return deps.agentTab.humanMay(tab ? tab.id : null);
}
