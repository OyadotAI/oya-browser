/**
 * Fixed values the tab code (main/tabs/) runs on: where a new tab opens, how
 * long a tab's protection may take, and the size of sign-in popups.
 */

/** Where a new tab, and a browser with no tabs left, opens. */
const HOME_URL = 'https://google.com';
/** A web address, as opposed to about:, file: or view-source:. */
const WEB_URL = /^https?:\/\//i;

/**
 * A CDP command that never answers must not strand the tab that is waiting on
 * it. Cloud browsers hit exactly that: setupTabCDP never resolved, so the
 * initial loadURL chained after it never ran — the tab sat on its start URL
 * with the title it was created with, and everything that waited on the tab
 * waited forever. Going ahead unprotected is bad; never loading a page is worse,
 * and the fail() above says so loudly either way.
 */
const CDP_SETUP_TIMEOUT = 10000;
/** A navigation Chromium abandoned for another one (ERR_ABORTED): not a failure worth showing. */
const ERR_ABORTED = -3;
/** A sign-in popup's window size. */
const AUTH_POPUP_SIZE = { width: 500, height: 700 };
/** White, under every page that sets no background of its own. */
const PAGE_BACKGROUND = '#ffffff';

module.exports = { HOME_URL, WEB_URL, CDP_SETUP_TIMEOUT, ERR_ABORTED, AUTH_POPUP_SIZE, PAGE_BACKGROUND };
