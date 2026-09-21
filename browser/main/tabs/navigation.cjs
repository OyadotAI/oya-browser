/**
 * The address bar's navigation of the active tab: pull the host's cookies,
 * wait for the tab's protection, then load, unless the person typed another
 * address, closed the tab, or lost control in the meantime.
 */
const { WEB_URL } = require('./constants.cjs');

/** Addresses typed without a scheme are https. */
function normalizeAddress(url) {
  const trimmed = String(url).trim();
  return WEB_URL.test(trimmed) ? trimmed : 'https://' + trimmed;
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
  if (ctx.control.snapshot().interactive) view.webContents.loadURL(url).catch(() => {});
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

module.exports = { navigateActive, normalizeAddress };
