/**
 * The Oya start page. A tab on it keeps its protected web view, so an agent can
 * drive it like any tab, but the view holds only its blank page and is not shown:
 * the shell draws the start page itself in that space, so nothing of it is ever
 * inside a website. The moment anything else happens in the view (a navigation
 * starts, even one that then fails, or an automation client writes content into
 * the blank page) the start page ends and the view is shown.
 */
const { HOME_URL } = require('./constants.cjs');

/** Whether `url` asks for the start page. */
const isHome = (url) => url === HOME_URL;

/** Whether `url` is the view's own blank page (or no address yet), not content. */
const isBlank = (url) => !url || /^about:blank\b/i.test(String(url));

/** The view a tab shows in the window: its page, or none while it is on the start page. */
const shownViewOf = (tab) => (tab && !tab.home ? tab.view : null);

/** Mounts what the tab shows in the shell window, unless an overlay holds the window. */
function mountTab(ctx, tab) {
  if (!ctx.overlays.names.size) ctx.shell.window.setBrowserView(shownViewOf(tab));
}

/** The tab is going to `url`: anything but its blank page ends the start page. Answers whether it just left it. */
function leaveHomeFor(tab, url) {
  if (!tab.home || isBlank(url)) return false;
  tab.home = false;
  return true;
}

/** Whether a tab on the start page should ignore this address or title: its blank page leaves it as it is. */
const staysHome = (tab, url) => !!tab.home && isBlank(url);

/** Where a tab is, as an address to reopen it at: the start page for one on it, about:blank for one that never loaded. */
const addressOf = (tab) => (tab.home ? HOME_URL : tab.url || 'about:blank');

/**
 * A document became ready in the view. After its blank page committed that is the
 * blank page itself; with no navigation before it, content was written into the
 * page (an automation client's setContent), which ends the start page. Answers
 * whether it just left it.
 */
function contentReady(tab) {
  const ownBlank = tab.blankCommitted;
  tab.blankCommitted = false;
  if (!tab.home || ownBlank) return false;
  tab.home = false;
  return true;
}

/** Follows the view, ending the start page as soon as the view holds anything but its blank page. */
function wireHome(ctx, tab) {
  const contents = tab.view.webContents;
  const show = () => tab.id === ctx.tabs.activeTabId && ctx.tabs.showInShell(tab);
  contents.on('did-start-navigation', (details) => details?.isMainFrame && leaveHomeFor(tab, details.url) && show());
  contents.on('did-navigate', (_e, url) => isBlank(url) && (tab.blankCommitted = true));
  contents.on('dom-ready', () => contentReady(tab) && show());
}

module.exports = { isHome, isBlank, shownViewOf, mountTab, leaveHomeFor, staysHome, addressOf, contentReady, wireHome };
