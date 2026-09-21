/**
 * What a tab listens to: its load state, its address and title, windows it
 * tries to open, and its right-click menu. Wired once, when the tab is made.
 */
const { sleep } = require('../input.cjs');
const { isAuthPopup, opensNamedWindow } = require('../auth-popup.cjs');
const { watchContents } = require('../observe/install.cjs');
const { showContextMenu } = require('./context-menu.cjs');
const { CDP_SETUP_TIMEOUT, ERR_ABORTED, AUTH_POPUP_SIZE } = require('./constants.cjs');

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
  wireTabLoadState(ctx, tab);
  wireTabFailures(ctx, tab);
  const tabReady = protectNewTab(ctx, tab.view);
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
    tab.loadError = null;
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
 * Attach CDP debugger and auto-inject scripts into every new document.
 * A view has no renderer until its first navigation, and CDP's Page domain
 * does not answer before there is one, so awaiting setup before loadURL was
 * a deadlock that only the timeout broke, and every tab's first page loaded
 * unprotected. about:blank starts the renderer without a network request.
 * The race does not cancel the losing sleep, so it checks whether setup
 * finished, otherwise every healthy tab reported itself unprotected.
 */
function protectNewTab(ctx, view) {
  const setup = { done: false };
  return Promise.race([
    setupAfterBlank(ctx, view, setup),
    sleep(CDP_SETUP_TIMEOUT).then(() => warnIfUnprotected(setup)),
  ]);
}

/** Starts the renderer on about:blank, then protects it. */
function setupAfterBlank(ctx, view, setup) {
  return view.webContents
    .loadURL('about:blank')
    .catch(() => {})
    .then(() => ctx.protection.setupTabCDP(view))
    .finally(() => (setup.done = true));
}

/** The timeout won the race: say so loudly unless setup did finish after all. */
function warnIfUnprotected(setup) {
  if (setup.done) return;
  console.error(
    `[anonymity] CDP setup unfinished after ${CDP_SETUP_TIMEOUT}ms, loading anyway, this tab may be UNPROTECTED`,
  );
}

/** A finished load, the address, the title, and joining a recording in progress. */
function wireTabPage(ctx, tab, tabReady) {
  const contents = tab.view.webContents;
  contents.on('did-finish-load', () => pageLoaded(ctx, tab.view));
  const updateUrl = (_e, u) => ctx.tabs.urlChanged(tab, u);
  contents.on('did-navigate', updateUrl);
  contents.on('did-navigate-in-page', updateUrl);
  // New tabs join an active recording before the user can interact with them.
  tabReady.then(() => ctx.recorder.joinIfRecording(tab.view)).catch((err) => console.error('[recording]', err));
  wireTabTitle(ctx, tab);
  if (ctx.observer) watchContents(ctx.observer, contents);
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
  const currentUrl = view.webContents.getURL();
  if (currentUrl.startsWith('view-source:')) {
    view.webContents.executeJavaScript(VIEW_SOURCE_LIGHT, true).catch(() => {});
  }
}

/** Windows the page opens, sign-in popups it is allowed, and its context menu. */
function wireTabWindows(ctx, tab) {
  const contents = tab.view.webContents;
  contents.setWindowOpenHandler((details) => openWindow(ctx, details));
  contents.on('did-create-window', (childWindow) => adoptPopup(ctx, childWindow));
  contents.on('context-menu', (_e, params) => showContextMenu(ctx, tab.view, params));
}

/**
 * An anonymous target="_blank" becomes a tab, which is what a person wants.
 * A sign-in popup and any window the page named stay real windows, because the
 * page holds on to what `window.open` gave it. Those are then adopted as tabs
 * (adoptPopup) so an agent can still list, switch to and drive them.
 */
function openWindow(ctx, details) {
  const webPreferences = { partition: ctx.persona.partitionName() };
  if (isAuthPopup(details.url, details.features))
    return { action: 'allow', overrideBrowserWindowOptions: { ...AUTH_POPUP_SIZE, webPreferences } };
  if (opensNamedWindow(details.frameName)) return { action: 'allow', overrideBrowserWindowOptions: { webPreferences } };
  ctx.tabs.createTab(details.url, true, loadOptionsFor(details));
  return { action: 'deny' };
}

/** Protects a window the page opened, and puts it on the tab list so it can be driven. */
function adoptPopup(ctx, childWindow) {
  ctx.protection.protectPopup(childWindow);
  ctx.tabs.adoptWindow?.(childWindow);
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
