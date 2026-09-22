/**
 * Fixed values the tab code (main/tabs/) runs on: where a new tab opens, how
 * long a tab's protection may take, and the size of sign-in popups.
 */

/** Where a new tab, and a browser with no tabs left, opens. */
const HOME_URL = 'https://google.com';
/** A web address, as opposed to about:, file: or view-source:. */
const WEB_URL = /^https?:\/\//i;
/** An address that reads this machine: a file: one, or the source view of one. */
const LOCAL_FILE = /^\s*(view-source:\s*)*file:/i;
/** Where the address bar sends text that is not an address, like any browser's. */
const SEARCH_URL = 'https://www.google.com/search?q=';

/**
 * How long one attempt at protecting a new tab may take. A CDP command that
 * never answers must not strand the tab waiting on it (cloud browsers hit
 * exactly that), so an attempt that runs out of time is reset and tried once
 * more; a second failure closes the tab to the web rather than loading a page
 * with no fingerprint, since a site that sees one real device has seen it for good.
 */
const CDP_SETUP_TIMEOUT = 10000;
/** What a command sent to a tab that could not be protected answers. */
const TAB_UNPROTECTED =
  'This tab could not be protected, so the page was not loaded. Nothing was sent to the site. Tried twice. Open a new tab and send the command again.';
/** What that tab shows the person. No Reload: a reload would load the page unprotected. */
const TAB_UNPROTECTED_DESKTOP =
  'This tab could not be protected, so the page was not loaded. Close it and open a new tab.';
/** A navigation Chromium abandoned for another one (ERR_ABORTED): not a failure worth showing. */
const ERR_ABORTED = -3;
/** A sign-in popup's window size. */
const AUTH_POPUP_SIZE = { width: 500, height: 700 };
/** White, under every page that sets no background of its own. */
const PAGE_BACKGROUND = '#ffffff';

module.exports = {
  HOME_URL,
  WEB_URL,
  LOCAL_FILE,
  SEARCH_URL,
  CDP_SETUP_TIMEOUT,
  TAB_UNPROTECTED,
  TAB_UNPROTECTED_DESKTOP,
  ERR_ABORTED,
  AUTH_POPUP_SIZE,
  PAGE_BACKGROUND,
};
