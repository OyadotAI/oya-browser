/**
 * A signed-in launch: the window opens straight to browsing, on a blank tab,
 * and the home page loads only once the server's cookies are in the jar (or
 * the server has not answered in RESUME_OFFLINE_MS). Session cookies are not
 * kept across a restart, so a site loaded any sooner minted a logged-out
 * cookie that then replaced the login the server's pool still held.
 */
const governance = require('../../governance');
const { HOME_URL } = require('../tabs/constants.cjs');
const { RESUME_OFFLINE_MS } = require('./constants.cjs');
const { loadInTab, isUnprotected, showUnprotected } = require('../tabs/load.cjs');

/** Addresses of a tab that has not loaded anything yet. */
const BLANK = new Set(['', 'about:blank']);

/**
 * Skips the welcome screen for a desktop that has signed in before. A governed
 * browser, or one never accepted (a fresh cloud sandbox), waits for the server:
 * it must not load a page before its rules or persona.
 */
function resumeSignedIn(ctx) {
  if (!ctx.config.values.apiKey || !ctx.persona.active || governance.configuration) return;
  ctx.tabs.enterBrowsingMode('about:blank');
  ctx.resumingHome = true;
  setTimeout(() => openResumedHome(ctx), RESUME_OFFLINE_MS).unref?.();
}

/**
 * Loads the home page in the resumed blank tab, once, through loadInTab like
 * every page (it waits for the tab's protection); a tab the person already
 * used is left alone.
 */
function openResumedHome(ctx) {
  if (!ctx.resumingHome) return;
  ctx.resumingHome = false;
  const tab = ctx.tabs.find(ctx.tabs.activeTabId);
  if (!tab || !BLANK.has(tab.view.webContents.getURL())) return;
  loadInTab(tab, HOME_URL).catch((e) => isUnprotected(e) && showUnprotected(ctx, tab));
}

module.exports = { resumeSignedIn, openResumedHome };
