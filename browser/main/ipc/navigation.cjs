/** IPC: the address bar, back/forward/reload, and the tab strip. */
const { HOME_URL } = require('../tabs/constants.cjs');

/** Throws unless a person holds control. */
const requireHuman = (ctx) => ctx.shield.requireHumanControl();

/** Channel → handler. */
const NAVIGATION_HANDLERS = {
  navigate: async (ctx, _e, url) => {
    requireHuman(ctx);
    if (!ctx.shell.browsingMode) return void ctx.tabs.enterBrowsingMode(url);
    await ctx.tabs.navigateActive(url);
  },
  'go-back': (ctx) => {
    requireHuman(ctx);
    ctx.tabs.getActiveView()?.webContents.goBack();
  },
  'go-forward': (ctx) => {
    requireHuman(ctx);
    ctx.tabs.getActiveView()?.webContents.goForward();
  },
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
