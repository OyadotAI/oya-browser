/**
 * Page zoom: the one place a person's zoom (View menu, shortcuts, the address bar's
 * chip) changes the shown page, and how the toolbar learns the zoom it shows.
 * Chromium keeps zoom per site, shared by every tab on it in every window, and tells
 * no other tab when one changes it, so the zoom is always read live, never cached.
 */
import type { WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';
import { shownViewOf } from '../tabs/home.ts';
import { FULL_SIZE_PERCENT, ZOOM_BASE } from './constants.ts';

/** What sending the zoom reads: the window's shown tab, and its shell. */
type SendDeps = Pick<AppServices, 'tabs' | 'shell'>;
/** What zooming needs: who may use the tab, and every window to tell. */
type ZoomDeps = SendDeps & Pick<AppServices, 'agentTab'> & Partial<Pick<AppServices, 'windows'>>;

/** A Chromium zoom level as the percentage people know from Chrome (level 0.5 is 110%). */
export function zoomPercent(level: number): number {
  return Math.round(FULL_SIZE_PERCENT * ZOOM_BASE ** level);
}

/**
 * The page this window shows, if it is a website's: the start page and the
 * library are the shell's, and a popup has its own window, so none is zoomed.
 */
function shownPage(deps: SendDeps): WebContents | null {
  const tab = deps.tabs.find(deps.tabs.activeTabId);
  const contents = tab && !tab.window ? (shownViewOf(tab)?.webContents as WebContents | undefined) : undefined;
  return contents && !contents.isDestroyed() ? contents : null;
}

/**
 * Tells this window's toolbar the shown tab's zoom; a page that is not a
 * website's reads 100. `announce` says a person just zoomed, so a screen reader
 * hears the new size.
 */
export function sendZoom(deps: SendDeps, announce: boolean): void {
  const level = shownPage(deps)?.getZoomLevel() ?? 0;
  deps.shell.send('page-zoom', { percent: zoomPercent(level), announce });
}

/**
 * Zooms the shown page to `level(current)`, only while the person may use it: a
 * tab an agent holds is never zoomed from here, though a person's tab on the same
 * site still moves it, as zoom is kept per site. The shell itself is never zoomed.
 * Every window is told, since another one may show a tab on the same site.
 */
export function zoomShownPage(deps: ZoomDeps, level: (current: number) => number): void {
  const contents = shownPage(deps);
  if (!contents || !deps.agentTab.humanMay(deps.tabs.activeTabId)) return;
  contents.setZoomLevel(level(contents.getZoomLevel()));
  if (deps.windows) deps.windows.each((scope) => sendZoom(scope, true));
  else sendZoom(deps, true);
}
