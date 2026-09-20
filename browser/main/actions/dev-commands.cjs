/**
 * The dev panel's quick actions. Command map: action → handler(driver, view,
 * params), each answering `{ ok, data?, error? }` by return value. Elements are
 * addressed by the analyzer's `data-ac-id`.
 */
const { cdp, cdpEval } = require('../cdp.cjs');
const { sleep, cdpPressKey, cdpTypeText, cdpClearField, cdpMouseMove, cdpClick, cdpScroll } = require('../input.cjs');
const { answerDialog } = require('../dialogs.cjs');
const { VIEWPORT_JS, DEV_ANALYZE_JS, devWaitJs, devSelectJs } = require('./scripts.cjs');
const { renderedAnalysis } = require('../page-format.cjs');
const c = require('./constants.cjs');

/** Actions that run without human control: they only read the page or the tab list. */
const UNGUARDED_DEV_COMMANDS = ['analyze', 'screenshot', 'list-tabs'];

/** The analyzer's selector for an element id. */
const byAcId = (elementId) => `[data-ac-id="${elementId}"]`;

/** One wheel event of `delta` at the viewport's centre. */
async function devScroll(view, delta) {
  const vp = await cdpEval(view, VIEWPORT_JS);
  await cdpScroll(view, (vp?.w || c.FALLBACK_VIEWPORT.w) / c.HALF, (vp?.h || c.FALLBACK_VIEWPORT.h) / c.HALF, 0, delta);
  return { ok: true };
}

/** The handler for each dev panel action. */
const DEV_COMMANDS = {
  /** The analyzer's full read of the page. */
  async analyze(driver, view) {
    await driver.ctx.injectScripts(view);
    return renderedAnalysis(driver.ctx, await driver.ctx.worldEval(view, DEV_ANALYZE_JS));
  },

  /**
   * Server-internal: the channel CAPTCHA and MFA handling use. It runs in
   * the PAGE's world, not the analyzer's isolated one — clearing a captcha
   * means calling back into the page's own globals
   * (`___grecaptcha_cfg.clients[…].callback` is a function the page
   * defined), which an isolated world cannot see. Not a public command.
   */
  async evaluate_raw(driver, view, params) {
    return { ok: true, data: { result: await cdpEval(view, String(params?.expression || '')) } };
  },

  /** A PNG of the active tab. */
  async screenshot(driver, view) {
    const r = await cdp(view, 'Page.captureScreenshot', { format: 'png' });
    return { ok: true, data: { screenshot: 'data:image/png;base64,' + r.data } };
  },

  /** Answers the open JavaScript dialog. */
  async handle_dialog(driver, view, params) {
    return await answerDialog(params?.accept, params?.prompt_text ?? params?.promptText);
  },

  /** Scrolls down by `amount` pixels. */
  'scroll-down': (driver, view, params) => devScroll(view, params?.amount || c.DEV_SCROLL_AMOUNT),

  /** Scrolls up by `amount` pixels. */
  'scroll-up': (driver, view, params) => devScroll(view, -(params?.amount || c.DEV_SCROLL_AMOUNT)),

  /** Reloads the page. */
  async reload(driver, view) {
    view.webContents.reload();
    return { ok: true };
  },

  /** Loads a URL, adding https:// when the scheme is missing. */
  async navigate(driver, view, params) {
    if (!params?.url) return { ok: false, error: 'URL required' };
    let url = params.url;
    if (!/^https?:\/\//i.test(url)) url = 'https://' + url;
    await driver.ctx.pullCookiesFor(url);
    await view.webContents.loadURL(url);
    await driver.ctx.injectScripts(view);
    return { ok: true, data: { url: view.webContents.getURL(), title: view.webContents.getTitle() } };
  },

  /** Clicks an element by id with the CDP mouse. */
  async click(driver, view, params) {
    if (!params?.element_id) return { ok: false, error: 'element_id required' };
    const info = await driver.locate(view, byAcId(params.element_id));
    if (!info?.ok) return { ok: false, error: info?.error || 'Element not found' };
    await cdpClick(view, info.data.x, info.data.y);
    await sleep(c.NAVIGATION_START_MS);
    return { ok: true, data: { clicked: true, url: view.webContents.getURL() } };
  },

  /** Clicks a field by id, clears it and types into it. */
  async type(driver, view, params) {
    if (!params?.element_id || !params?.text) return { ok: false, error: 'element_id and text required' };
    const info = await driver.locate(view, byAcId(params.element_id));
    if (!info?.ok) return { ok: false, error: info?.error || 'Element not found' };
    await cdpClick(view, info.data.x, info.data.y);
    await sleep(c.DEV_FOCUS_MS);
    await cdpClearField(view);
    await cdpTypeText(view, params.text);
    return { ok: true, data: { typed: true } };
  },

  /** Presses one key with the CDP keyboard. */
  async 'press-key'(driver, view, params) {
    if (!params?.key) return { ok: false, error: 'key required' };
    await cdpPressKey(view, params.key);
    return { ok: true, data: { key: params.key } };
  },

  /** Moves the mouse onto an element by id. */
  async hover(driver, view, params) {
    if (!params?.element_id) return { ok: false, error: 'element_id required' };
    const info = await driver.locate(view, byAcId(params.element_id));
    if (!info?.ok) return { ok: false, error: info?.error || 'Element not found' };
    await cdpMouseMove(view, Math.round(info.data.x), Math.round(info.data.y));
    return { ok: true, data: { hovered: true } };
  },

  /** A CDP click at page coordinates. */
  async 'click-coords'(driver, view, params) {
    if (params?.x == null || params?.y == null) return { ok: false, error: 'x and y required' };
    await cdpClick(view, params.x, params.y);
    return { ok: true, data: { clicked: true, x: params.x, y: params.y } };
  },

  /** Waits for a CSS selector to match. */
  async wait(driver, view, params) {
    if (!params?.selector) return { ok: false, error: 'selector required' };
    await driver.ctx.injectScripts(view);
    return await driver.ctx.worldEval(view, devWaitJs(params), true);
  },

  /** The open tabs. */
  async 'list-tabs'(driver) {
    const { ctx } = driver;
    return {
      ok: true,
      data: {
        tabs: ctx.tabs().map((t) => ({ id: t.id, title: t.title, url: t.url, active: t.id === ctx.activeTabId() })),
      },
    };
  },

  /** Opens a tab. */
  async 'new-tab'(driver, view, params) {
    const tabId = driver.ctx.createTab(params?.url || 'about:blank', true);
    return { ok: true, data: { tab_id: tabId } };
  },

  /** Closes a tab, the active one by default. */
  async 'close-tab'(driver, view, params) {
    driver.ctx.closeTab(params?.tab_id || driver.ctx.activeTabId());
    return { ok: true, data: { closed: true } };
  },

  /** Sets a <select>'s value by element id. */
  async select(driver, view, params) {
    if (!params?.element_id || !params?.value) return { ok: false, error: 'element_id and value required' };
    await driver.ctx.injectScripts(view);
    return await driver.ctx.worldEval(view, devSelectJs(params), true);
  },
};

module.exports = { DEV_COMMANDS, UNGUARDED_DEV_COMMANDS };
