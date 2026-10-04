/**
 * A tab's favicon for the strip. The page names its icons; the icon is
 * fetched here, through the tab's own session (the persona's proxy and
 * cookies, like any request the page makes), and handed to the shell as a
 * data: URL. The shell page never fetches it itself: its session is not the
 * persona's, so a request from it would leave the persona's network.
 */
import type { Session } from 'electron';
import { FAVICON_MAX_BYTES, FAVICON_URL } from './constants.ts';
import type { Tab, TabListSink } from './types.ts';

/** The origin of `url`, or '' when it has none. */
function siteOrigin(url: string): string {
  try {
    return new URL(url).origin;
  } catch {
    return '';
  }
}

/** The icon's bytes as a data: URL, or null when it is not an image or too big to be an icon. */
export async function fetchIcon(session: Pick<Session, 'fetch'>, url: string): Promise<string | null> {
  const response = await session.fetch(url);
  const type = response.headers.get('content-type') || '';
  if (!response.ok || !type.startsWith('image/')) return null;
  const bytes = Buffer.from(await response.arrayBuffer());
  if (!bytes.length || bytes.length > FAVICON_MAX_BYTES) return null;
  return `data:${type.split(';')[0]};base64,${bytes.toString('base64')}`;
}

/** Shows the page's first icon on the tab, unless the page moved on while it was fetched. */
async function iconChanged(tabs: TabListSink, tab: Tab, url: string | undefined): Promise<void> {
  if (!url || !FAVICON_URL.test(url) || url === tab.faviconUrl) return;
  tab.faviconUrl = url;
  const data = await fetchIcon(tab.view.webContents.session, url).catch(() => null);
  if (tab.faviconUrl !== url) return;
  tab.favicon = data;
  tabs.sendTabList();
}

/** A page on another site has no icon until it names one; the same site keeps its icon, so it does not flicker. */
function pageChanged(tabs: TabListSink, tab: Tab, url: string): void {
  const origin = siteOrigin(url);
  if (origin === tab.faviconOrigin) return;
  tab.faviconOrigin = origin;
  if (!tab.faviconUrl && !tab.favicon) return;
  Object.assign(tab, { faviconUrl: null, favicon: null });
  tabs.sendTabList();
}

/** Listens for the tab's icons and for it leaving the site they belong to; `tabs` is told when the icon changes. */
export function wireFavicon(tabs: TabListSink, tab: Tab): void {
  const contents = tab.view.webContents;
  contents.on('page-favicon-updated', (_e, icons) => iconChanged(tabs, tab, icons?.[0]));
  contents.on('did-navigate', (_e, url) => pageChanged(tabs, tab, url));
}
