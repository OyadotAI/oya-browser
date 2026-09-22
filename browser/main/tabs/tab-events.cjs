/**
 * What a tab listens to: its load state, its address and title, windows it
 * tries to open, and its right-click menu. Wired once, when the tab is made.
 */
const { withinTime } = require('../../scripts/within-time.cjs');
const { isAuthPopup, opensNamedWindow } = require('../auth-popup.cjs');
const { watchContents } = require('../observe/install.cjs');
const { showContextMenu } = require('./context-menu.cjs');
const { showUnprotected } = require('./load.cjs');
const { trackFrameSessions } = require('../recording/frame-sessions.cjs');
const { pageReached } = require('../recording/outcomes.cjs');
const { CDP_SETUP_TIMEOUT, ERR_ABORTED, AUTH_POPUP_SIZE, LOCAL_FILE } = require('./constants.cjs');

/**
 * Makes view-source pages readable (forces the light theme). This text runs
 * in the page, so it is kept exactly as it has always been.
 */
const VIEW_SOURCE_LIGHT = `
        document.documentElement.style.cssText = 'background:#fff!important;color:#000!important;color-scheme:light!important';
        document.body.style.cssText = 'background:#fff!important;color:#000!important';
        const s = document.createElement('style');
        s.textContent = '*, *::before, *::after { color-scheme: light !important; } body, html, .line-content, .line-number, td, tr, table { background-color: #fff !important; color: #000 !important; } a { color: #00e !important; }';
        document.head.appendChild(s);
      `;

/** Wires every listener on a new tab; returns the promise that settles once it is protected (or given up on). */
function wireTab(ctx, tab) {
  wireRecording(ctx, tab);
  wireTabLoadState(ctx, tab);
  wireTabFailures(ctx, tab);
  wireRendererLoss(ctx, tab);
  const tabReady = protectNewTab(ctx, tab);
  wireTabPage(ctx, tab, tabReady);
  wireTabWindows(ctx, tab);
  return tabReady;
}

/** Shortcuts, focus, and the loading / failed state the tab strip shows. */
function wireTabLoadState(ctx, tab) {
  const contents = tab.view.webContents;
  ctx.shortcuts.install(contents);
  contents.on('focus', () => ctx.shield.keepFocusOnShell());
  contents.on('did-start-loading', () => {
    // A tab that could not be protected keeps saying so: nothing it starts will load.
    if (tab.protection !== 'failed') tab.loadError = null;
    ctx.tabs.sendTabList();
  });
  contents.on('did-stop-loading', () => ctx.tabs.sendTabList());
}

/** Puts an error on the tab and stops its spinner. */
function showTabError(ctx, tab, message) {
  tab.navigationPending = false;
  tab.loadError = message;
  ctx.tabs.sendTabList();
}

/** A dead renderer's recording channel died with it; forgotten, the reload's page arms a new one. */
function wireRendererLoss(ctx, tab) {
  tab.view.webContents.on('render-process-gone', () => ctx.recorder.channels.forget(tab.view));
}

/** A page that could not load, or a renderer that died, is shown on the tab. */
function wireTabFailures(ctx, tab) {
  const failed = (message) => showTabError(ctx, tab, message);
  tab.view.webContents.on('did-fail-load', (_event, code, description, _url, mainFrame) => {
    if (!mainFrame || code === ERR_ABORTED) return;
    failed(`Page could not load: ${description}. Try Reload.`);
  });
  tab.view.webContents.on('render-process-gone', (_event, details) => {
    failed(`Page renderer stopped (${details.reason}). Reload to recover.`);
  });
}

/**
 * Attach CDP debugger and auto-inject scripts into every new document, then
 * settle `tab.protection`. A view has no renderer until its first navigation,
 * and CDP's Page domain does not answer before there is one, so setup runs
 * after about:blank, which starts the renderer without a network request.
 * An attempt that fails or runs out of time is reset and tried once more; a
 * second failure leaves the tab on about:blank, closed to the web, since a
 * page loaded with no fingerprint shows the site this machine for good.
 * Always settles, within two attempts.
 */
async function protectNewTab(ctx, tab) {
  tab.protection = 'pending';
  const blank = tab.view.webContents.loadURL('about:blank').catch(() => {});
  if (await protectOnce(ctx, tab.view, blank)) return markProtected(tab);
  ctx.protection.resetTabCDP(tab.view);
  console.error('[anonymity] tab protection did not finish on the first attempt, trying once more');
  if (await protectOnce(ctx, tab.view, blank)) return markProtected(tab);
  failClosed(ctx, tab);
}

/** The tab is protected: pages may load in it. */
function markProtected(tab) {
  tab.protection = 'protected';
}

/** One bounded attempt: the blank page, then setup. True only when setup says the tab is protected. */
const protectOnce = (ctx, view, blank) =>
  withinTime(
    blank.then(() => ctx.protection.setupTabCDP(view)),
    CDP_SETUP_TIMEOUT,
  ).then(
    (ok) => ok === true,
    () => false,
  );

/** Both attempts failed: nothing is loaded in this tab, and the person is told why. */
function failClosed(ctx, tab) {
  ctx.protection.resetTabCDP(tab.view);
  tab.protection = 'failed';
  console.error('[anonymity] tab protection failed twice, tab not loaded');
  showUnprotected(ctx, tab);
}

/** A finished load, the address, the title, and joining a recording in progress. */
function wireTabPage(ctx, tab, tabReady) {
  const contents = tab.view.webContents;
  contents.on('did-finish-load', () => pageLoaded(ctx, tab.view));
  const updateUrl = (_e, u) => ctx.tabs.urlChanged(tab, u);
  contents.on('did-navigate', updateUrl);
  contents.on('did-navigate-in-page', updateUrl);
  // New tabs join an active recording before the user can interact with them; one never protected has nothing to record.
  tabReady.then(() => tab.protection === 'protected' && joinRecording(ctx, tab.view));
  wireTabTitle(ctx, tab);
  if (ctx.observer) watchContents(ctx.observer, contents);
}

/**
 * What a recording needs from the tab: its cross-site iframes, tracked before
 * protection attaches to them so a later recording finds them, and a page check
 * when a person's action moves the page (recording/outcomes.cjs).
 */
function wireRecording(ctx, tab) {
  trackFrameSessions(tab.view);
  const contents = tab.view.webContents;
  contents.on('did-navigate', (_e, u) => pageReached(ctx.recorder, tab.id, u));
  contents.on('did-navigate-in-page', (_e, u, isMainFrame) => isMainFrame && pageReached(ctx.recorder, tab.id, u));
}

/** Joins a recording in progress; a page that refuses is logged and tried again on its next load. */
function joinRecording(ctx, view) {
  return Promise.resolve(ctx.recorder.joinIfRecording(view)).catch((err) => console.error('[recording]', err.message));
}

/**
 * The tab's title. A page with no <title> (about:blank) never fires
 * page-title-updated, and the tab kept the previous page's name; reading it
 * again on every load fixes that, since getTitle() falls back to the address.
 */
function wireTabTitle(ctx, tab) {
  const contents = tab.view.webContents;
  contents.on('page-title-updated', (_e, title) => ctx.tabs.titleChanged(tab, title));
  contents.on('did-finish-load', () => ctx.tabs.titleChanged(tab, contents.getTitle()));
}

/** Loads the analyzer, and lightens view-source pages. */
function pageLoaded(ctx, view) {
  ctx.protection.injectScripts(view);
  // A tab that could not be armed, or whose renderer died, joins the recording again here.
  joinRecording(ctx, view);
  const currentUrl = view.webContents.getURL();
  if (currentUrl.startsWith('view-source:')) {
    view.webContents.executeJavaScript(VIEW_SOURCE_LIGHT, true).catch(() => {});
  }
}

/** Windows the page opens, sign-in popups it is allowed, and its context menu. */
function wireTabWindows(ctx, tab) {
  const contents = tab.view.webContents;
  contents.setWindowOpenHandler((details) => openWindow(ctx, details, tab));
  contents.on('did-create-window', (childWindow) => adoptPopup(ctx, childWindow));
  contents.on('context-menu', (_e, params) => showContextMenu(ctx, tab.view, params));
}

/**
 * An anonymous target="_blank" becomes a tab, which is what a person wants.
 * A sign-in popup and any window the page named stay real windows, because the
 * page holds on to what `window.open` gave it. Those are then adopted as tabs
 * (adoptPopup) so an agent can still list, switch to and drive them.
 */
function openWindow(ctx, details, opener) {
  const webPreferences = { partition: ctx.persona.partitionName() };
  if (isAuthPopup(details.url, details.features))
    return { action: 'allow', overrideBrowserWindowOptions: { ...AUTH_POPUP_SIZE, webPreferences } };
  if (opensNamedWindow(details.frameName)) return { action: 'allow', overrideBrowserWindowOptions: { webPreferences } };
  // A page cannot load a file: address itself, but a tab the app opens for it could: the page must not get one that way.
  if (!LOCAL_FILE.test(details.url))
    markOpener(ctx, ctx.tabs.createTab(details.url, true, loadOptionsFor(details)), opener);
  return { action: 'deny' };
}

/** Notes which tab opened a tab, so what a test run's tabs open closes with them. */
function markOpener(ctx, id, opener) {
  const tab = ctx.tabs.list.find((t) => t.id === id);
  if (tab && opener) tab.openerId = opener.id;
}

/** Protects a window the page opened, and puts it on the tab list so it can be driven. */
function adoptPopup(ctx, childWindow) {
  ctx.protection.protectPopup(childWindow);
  ctx.tabs.adoptWindow?.(childWindow);
  // A sign-in popup is part of the task: what the person types there is recorded too.
  const adopted = ctx.tabs.list.find((t) => t.window === childWindow);
  if (adopted) joinRecording(ctx, adopted.view);
}

/**
 * The new tab loads the page the way the page asked for it. A form that posts
 * into a new window (a SAML single sign-on, say) sends a body, and servers check
 * the referrer: reloading the URL with a bare GET lost both, and such links
 * answered 403 Forbidden.
 */
function loadOptionsFor({ referrer, postBody }) {
  const options = {};
  if (referrer?.url) options.httpReferrer = referrer;
  if (postBody?.data?.length) {
    const type = postBody.boundary ? `${postBody.contentType}; boundary=${postBody.boundary}` : postBody.contentType;
    Object.assign(options, { postData: postBody.data, extraHeaders: `Content-Type: ${type}` });
  }
  return options;
}

module.exports = { wireTab, VIEW_SOURCE_LIGHT };
