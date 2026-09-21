/**
 * The dev panel's quick actions. Command map: action → handler(driver, view,
 * params), each answering `{ ok, data?, error? }` by return value. Elements are
 * addressed by the analyzer's `data-ac-id`.
 */
const { cdp, cdpEval } = require('../cdp.cjs');
const { sleep, cdpPressKey, cdpTypeText, cdpClearField, cdpMouseMove, cdpClick, cdpScroll } = require('../input.cjs');
const { VIEWPORT_JS, DEV_ANALYZE_JS, devWaitJs } = require('./scripts.cjs');
const { HOME_URL } = require('../tabs/constants.cjs');
const { renderedAnalysis } = require('../page-format.cjs');
const c = require('./constants.cjs');

/**
 * Actions that run without human control: they only look. Analyze is not one:
 * it renumbers the page's element ids, which would break the agent's next click.
 */
const UNGUARDED_DEV_COMMANDS = ['screenshot', 'list-tabs'];

/** The analyzer's selector for an element id. */
const byAcId = (elementId) => `[data-ac-id="${elementId}"]`;

/** Why `params` has no usable element number, or null when it has one. */
function elementIdProblem(params) {
  if (!params?.element_id) return 'element_id required';
  return /^\d+$/.test(String(params.element_id)) ? null : 'element_id must be a number';
}

/** Whether `value` is a real number: an empty field is not 0. */
const isCoordinate = (value) => value !== '' && value !== null && Number.isFinite(Number(value));

/** Clicks into the field at `point`, then empties it. */
async function focusAndClear(view, point) {
  await cdpClick(view, point.x, point.y);
  await sleep(c.DEV_FOCUS_MS);
  await cdpClearField(view);
}

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

  /** A PNG of the active tab. */
  async screenshot(driver, view) {
    const r = await cdp(view, 'Page.captureScreenshot', { format: 'png' });
    return { ok: true, data: { screenshot: 'data:image/png;base64,' + r.data } };
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

  /** Goes to an address the way the address bar does (its search, cookies and recording included). */
  async navigate(driver, view, params) {
    if (!params?.url) return { ok: false, error: 'URL required' };
    await driver.ctx.navigate(params.url);
    return { ok: true, data: { url: view.webContents.getURL(), title: view.webContents.getTitle() } };
  },

  /** Clicks an element by id with the CDP mouse. */
  async click(driver, view, params) {
    const problem = elementIdProblem(params);
    if (problem) return { ok: false, error: problem };
    const info = await driver.locate(view, byAcId(params.element_id));
    if (!info?.ok) return { ok: false, error: info?.error || 'Element not found' };
    await cdpClick(view, info.data.x, info.data.y);
    await sleep(c.NAVIGATION_START_MS);
    return { ok: true, data: { clicked: true, url: view.webContents.getURL() } };
  },

  /** Clicks a field by id, clears it and types into it. */
  async type(driver, view, params) {
    const problem = params?.text ? elementIdProblem(params) : 'element_id and text required';
    if (problem) return { ok: false, error: problem };
    const info = await driver.locate(view, byAcId(params.element_id));
    if (!info?.ok) return { ok: false, error: info?.error || 'Element not found' };
    await focusAndClear(view, info.data);
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
    const problem = elementIdProblem(params);
    if (problem) return { ok: false, error: problem };
    const info = await driver.locate(view, byAcId(params.element_id));
    if (!info?.ok) return { ok: false, error: info?.error || 'Element not found' };
    await cdpMouseMove(view, Math.round(info.data.x), Math.round(info.data.y));
    return { ok: true, data: { hovered: true } };
  },

  /** A CDP click at page coordinates. */
  async 'click-coords'(driver, view, params) {
    if (!isCoordinate(params?.x) || !isCoordinate(params?.y)) return { ok: false, error: 'x and y required' };
    const [x, y] = [Number(params.x), Number(params.y)];
    await cdpClick(view, x, y);
    return { ok: true, data: { clicked: true, x, y } };
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

  /** Opens a tab on the home page, like the new tab button. */
  async 'new-tab'(driver, view, params) {
    const tabId = driver.ctx.createTab(params?.url || HOME_URL, true);
    return { ok: true, data: { tab_id: tabId } };
  },
};

module.exports = { DEV_COMMANDS, UNGUARDED_DEV_COMMANDS };
