/**
 * The address bar's navigation of the active tab: pull the host's cookies,
 * wait for the tab's protection, then load, unless the person typed another
 * address, closed the tab, or lost control in the meantime.
 */
const { WEB_URL, SEARCH_URL } = require('./constants.cjs');
const { loadInTab, isUnprotected, showUnprotected } = require('./load.cjs');

/** A host, with an optional port and path: has a dot, or is localhost, and no spaces. */
const HOST = /^(localhost|[^\s/:]+\.[^\s/:]+)(:\d+)?(\/\S*)?$/i;
/** This machine or a bare IP, which usually serves plain http. */
const PLAIN_HTTP = /^(localhost|127\.|\d{1,3}(\.\d{1,3}){3}[:/]?|\[::1\])/i;

/**
 * Whether a tab may be sent here: an http(s) address or about:blank. file: reads
 * this machine, javascript: runs as code in the page, and both the person's
 * address bar and a caller's command go through this one rule.
 */
function isWebAddress(url) {
  const trimmed = String(url).trim();
  return WEB_URL.test(trimmed) || trimmed.toLowerCase() === 'about:blank';
}

/** What a command is told when it asks for any other address. */
const NOT_A_WEB_ADDRESS = 'Only http and https addresses, or about:blank, can be opened.';

/**
 * What the address bar loads for what was typed, as any browser does: an http(s)
 * address as it is, a bare host over https (http for this machine or an IP),
 * about:blank, and anything else as a search. Other schemes (file:, javascript:)
 * are searched, never opened: the bar must not reach local files.
 */
function normalizeAddress(url) {
  const trimmed = String(url).trim();
  if (isWebAddress(trimmed)) return trimmed;
  if (HOST.test(trimmed)) return (PLAIN_HTTP.test(trimmed) ? 'http://' : 'https://') + trimmed;
  return SEARCH_URL + encodeURIComponent(trimmed);
}

/** Marks a tab as navigating; returns the request number that must still be current at the end. */
function beginNavigation(ctx, tab) {
  const request = (tab.navigationRequest = (tab.navigationRequest || 0) + 1);
  tab.navigationPending = true;
  tab.loadError = null;
  ctx.tabs.sendTabList();
  return request;
}

/** Loads the page, unless the tab closed, another navigation took over, or control moved. */
function finishNavigation(ctx, tab, url, request) {
  const view = tab.view;
  if (view.webContents.isDestroyed() || tab.navigationRequest !== request) return;
  tab.navigationPending = false;
  if (ctx.control.snapshot().interactive)
    loadInTab(tab, url).catch((e) => isUnprotected(e) && showUnprotected(ctx, tab));
  ctx.tabs.sendTabList();
}

/** Navigates the active tab to what the person typed. */
async function navigateActive(ctx, url) {
  const view = ctx.tabs.getActiveView();
  if (!view) return;
  const tab = ctx.tabs.list.find((t) => t.view === view);
  const request = beginNavigation(ctx, tab);
  url = normalizeAddress(url);
  ctx.recorder.recordNavigation(url);
  await Promise.all([ctx.cookies.pullCookiesFor(url), tab.setup]);
  finishNavigation(ctx, tab, url, request);
}

module.exports = { navigateActive, normalizeAddress, isWebAddress, NOT_A_WEB_ADDRESS };
