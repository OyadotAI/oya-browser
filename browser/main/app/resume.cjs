/**
 * A signed-in launch: the window opens straight to browsing, on the Oya start
 * page, which loads nothing from the web. Session cookies are not kept across a
 * restart, so a site loaded before the server's cookies are in the jar minted a
 * logged-out cookie that then replaced the login the server's pool still held;
 * the start page cannot, so the person can begin at once.
 */
const governance = require('../../governance');
const { HOME_URL } = require('../tabs/constants.cjs');

/**
 * Skips the welcome screen for a desktop that has signed in before. A governed
 * browser, or one never accepted (a fresh cloud sandbox), waits for the server:
 * it must not load a page before its rules or persona.
 */
function resumeSignedIn(ctx) {
  if (!ctx.config.values.apiKey || !ctx.persona.active || governance.configuration) return;
  ctx.tabs.enterBrowsingMode(HOME_URL);
}

module.exports = { resumeSignedIn };
