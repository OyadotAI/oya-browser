/** IPC: the address bar, back/forward/reload, and the tab strip. */
const { HOME_URL } = require('../tabs/constants.cjs');

/** Throws unless a person holds control. */
const requireHuman = (ctx) => ctx.shield.requireHumanControl();

/** Back or forward in the active tab, recorded as a step when a recording is running. */
async function goInHistory(ctx, action) {
  requireHuman(ctx);
  const contents = ctx.tabs.getActiveView()?.webContents;
  const back = action === 'go_back';
  const history = contents?.navigationHistory;
  if (!contents || (history && !(back ? history.canGoBack() : history.canGoForward()))) return;
  await ctx.recorder.recordHistory(action);
  if (back) contents.goBack();
  else contents.goForward();
}

/** Channel → handler. */
const NAVIGATION_HANDLERS = {
  navigate: async (ctx, _e, url) => {
    requireHuman(ctx);
    if (!ctx.shell.browsingMode) return void ctx.tabs.enterBrowsingMode(url);
    await ctx.tabs.navigateActive(url);
  },
  'go-back': (ctx) => goInHistory(ctx, 'go_back'),
  'go-forward': (ctx) => goInHistory(ctx, 'go_forward'),
  reload: (ctx) => ctx.tabs.reloadActivePage(),
  'enter-browsing': (ctx) => ctx.tabs.enterBrowsingMode(HOME_URL),
  'new-tab': (ctx, _e, url) => {
    requireHuman(ctx);
    const id = ctx.tabs.createTab(url || HOME_URL, true);
    ctx.recorder.recordNavigation(url || HOME_URL);
    return id;
  },
  'close-tab': (ctx, _e, id) => {
    requireHuman(ctx);
    ctx.tabs.closeTab(id);
  },
  'activate-tab': (ctx, _e, id) => ctx.tabs.activateTab(id),
};

module.exports = { NAVIGATION_HANDLERS };
