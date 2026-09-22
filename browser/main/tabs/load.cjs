/**
 * The one way a tab loads a page: after its protection has settled, and never
 * in a tab that could not be protected. The first page, the address bar and a
 * navigate command all come through here, so none of them can load a page
 * with no fingerprint while setup is still pending or after it failed.
 */
const { TAB_UNPROTECTED, TAB_UNPROTECTED_DESKTOP } = require('./constants.cjs');

/** The code a caller branches on, carried on the error to the server. */
const UNPROTECTED_CODE = 'tab_unprotected';

/** The refusal for a tab that could not be protected. */
function unprotected() {
  const err = new Error(TAB_UNPROTECTED);
  err.code = UNPROTECTED_CODE;
  return err;
}

/** Whether `err` is that refusal. */
const isUnprotected = (err) => err?.code === UNPROTECTED_CODE;

/**
 * Resolves once the tab's protection has settled; throws for a tab it failed
 * on. `tab.setup` always settles (two bounded attempts), and a tab the app
 * adopted from a popup has none, so it goes on as it always has.
 */
async function whenProtected(tab) {
  await tab?.setup;
  if (tab?.protection === 'failed') throw unprotected();
}

/** Loads `url` once the tab's protection has settled; refuses a tab it failed on. */
async function loadInTab(tab, url, options) {
  await whenProtected(tab);
  return tab.view.webContents.loadURL(url, options);
}

/** Shows the person why the tab stays empty, and offers no Reload. */
function showUnprotected(ctx, tab) {
  tab.navigationPending = false;
  tab.loadError = TAB_UNPROTECTED_DESKTOP;
  ctx.tabs.sendTabList();
}

module.exports = { loadInTab, whenProtected, unprotected, isUnprotected, showUnprotected };
