/**
 * Server commands that act on the page's content: navigate, screenshot, and
 * element clicks, typing, keys, hovers and selects. Command map: action →
 * handler(driver, id, params, view); each answers through sendResult.
 * Pointer and raw keyboard commands are in pointer-commands.cjs.
 */
const { cdp, cdpEval } = require('../cdp.cjs');
const { sleep, cdpPressKey, cdpTypeText, cdpClick, cdpMouseMove } = require('../input.cjs');
const { jitter } = require('../input/timing.cjs');
const s = require('./scripts.cjs');
const c = require('./constants.cjs');
const { POINTER_COMMANDS } = require('./pointer-commands.cjs');
const { isDateInput, dateInputValue, unreadableDate } = require('../../scripts/date-value.cjs');
const { isWebAddress, NOT_A_WEB_ADDRESS } = require('../tabs/navigation.cjs');
const { loadInTab, isUnprotected } = require('../tabs/load.cjs');

/** Keys that zoom, open the emoji picker, or trigger OS shortcuts. */
const BLOCKED_KEYS = new Set([
  'F11',
  'F12',
  'F5',
  // bare modifier keys
  'Meta',
  'Control',
  'Alt',
  'Shift',
  'ZoomIn',
  'ZoomOut',
  'BrowserBack',
  'BrowserForward',
  'MediaPlayPause',
  'MediaTrackNext',
  'MediaTrackPrevious',
  'AudioVolumeUp',
  'AudioVolumeDown',
  'AudioVolumeMute',
]);

/** The tab showing `view`, once its first load settles (or is given up on); a view no tab owns stands alone. */
async function readyTab(driver, view) {
  const tab = driver.ctx.tabs().find((t) => t.view === view) ?? { view };
  await driver.waitForTabReady(tab);
  return tab;
}

/** One load of `url`: null when it loaded, otherwise the error. A tab that is not protected is never retried: it throws. */
async function attemptLoad(tab, url) {
  try {
    await loadInTab(tab, url);
    return null;
  } catch (navErr) {
    if (isUnprotected(navErr)) throw navErr;
    return navErr;
  }
}

/** Loads `url`, retrying real failures; resolves to the last error, or null once loaded or aborted. */
async function loadWithRetries(tab, url) {
  for (let attempt = 0; ; attempt++) {
    const navErr = await attemptLoad(tab, url);
    if (!navErr || navErr.message?.includes('ERR_ABORTED')) return null;
    if (attempt === c.NAVIGATE_RETRIES) return navErr;
    await sleep(c.NAVIGATE_RETRY_MS);
  }
}

/**
 * The handles a replay finds this element by again, when the finder read them.
 * Reported from here because here the element is unambiguous, an id from an
 * earlier analysis may name nothing by the time the step is recorded.
 */
const withHandle = (info) => (info?.data?.handle ? { handle: info.data.handle } : {});

/** Gives a click or Enter time to start a navigation and waits it out; true when there was one. */
async function settleNavigation(driver, view) {
  await sleep(c.NAVIGATION_START_MS);
  if (!view.webContents.isLoading()) return false;
  await driver.waitForLoad(view);
  return true;
}

/** Waits out any navigation a click started, then reads where the page landed and reloads the analyzer. */
async function landedPage(driver, view) {
  await settleNavigation(driver, view);
  const url = view.webContents.getURL();
  const title = view.webContents.getTitle();
  await driver.ctx.injectScripts(view);
  return { url, title };
}

/**
 * Find element and click on it (natural focus, like a human clicking the
 * field), then pause before typing. Resolves to the element, or null once a
 * missing one is answered.
 */
async function focusField(driver, id, view, selector) {
  const info = await driver.find(id, view, selector);
  if (!info) return null;
  await cdpClick(view, info.data.x, info.data.y);
  await sleep(jitter(c.FOCUS_PAUSE));
  return info;
}

/** The field's current text, or null when it cannot be read. */
const fieldValue = (driver, view, selector) =>
  driver.ctx.worldEval(view, s.fieldValueJs(selector), true).catch(() => null);

/** Whether a field holds text to clear. An unfilled mask such as __/__/____ holds none. */
const hasContent = (value) => typeof value !== 'string' || /[\p{L}\p{N}]/u.test(value);

/**
 * Empties a field before typing. A filled one is selected and deleted with a
 * real Backspace (`press` sends it to the right frame), then given a moment:
 * masked fields redraw after a clear and drop keys typed meanwhile. An unfilled
 * mask is left alone, with the caret moved to its start, where the first
 * character belongs: a click in its middle leaves the caret there.
 */
async function clearField(driver, view, selector, press) {
  if (!hasContent(await fieldValue(driver, view, selector)))
    return driver.ctx.worldEval(view, s.caretStartJs(selector), true).catch(() => {});
  const selected = await driver.ctx.worldEval(view, s.selectFieldJs(selector), true).catch(() => false);
  if (selected !== 'select') return;
  await sleep(jitter(c.CLEAR_PAUSE));
  await press(view, 'Backspace');
  await sleep(c.CLEAR_SETTLE_MS);
}

/**
 * Clear existing content, use JS to target the specific element instead of
 * CDP Cmd+A which can select the entire page, then type with human cadence.
 *
 * One path for the page and for a frame inside it. A second path existed for
 * iframes, on the belief that CDP keyboard events do not reach them; they do,
 * clicking an input inside an iframe and typing through CDP puts the characters in
 * it and fires the frame's own keydown. What the frame path actually did was
 * nothing, in a frame or out of it, so typing into an iframe silently did nothing
 * at all. Real key events still matter for masked fields, and these are real ones.
 */
async function typeIntoField(driver, view, selector, text) {
  await clearField(driver, view, selector, cdpPressKey);
  await cdpTypeText(view, text);
}

/**
 * A native date or time input gets its value set, not typed: typed digits land
 * in whichever segment has focus. Answers false for any other field.
 */
async function fillIfDate(driver, id, view, selector, text) {
  const type = await driver.ctx.worldEval(view, s.inputTypeJs(selector), true).catch(() => null);
  if (!isDateInput(type)) return false;
  const value = dateInputValue(type, text);
  if (value === null) driver.ctx.sendResult(id, false, null, unreadableDate(type, text));
  else await setDate(driver, id, view, selector, { type, value });
  return true;
}

/** Sets the value and reports it, or says the field refused it (outside its min or max, say). */
async function setDate(driver, id, view, selector, { type, value }) {
  const kept = await driver.ctx.worldEval(view, s.setInputValueJs(selector, value), true).catch(() => null);
  if (kept === value) return driver.ctx.sendResult(id, true, { typed: true, value });
  driver.ctx.sendResult(id, false, null, `The ${type} field did not accept ${value}`);
}

/** Types into a text field, then reports whether suggestions appeared. */
async function typeAndReport(driver, id, view, { selector, text, handle }) {
  await typeIntoField(driver, view, selector, text);
  const suggestions = await suggestionsVisible(driver, view);
  driver.ctx.sendResult(id, true, {
    typed: true,
    suggestions_visible: suggestions,
    ...(handle ? { handle } : {}),
    ...shownIfChanged(await fieldValue(driver, view, selector), text),
  });
}

/** What the field shows when it is not what was typed (a mask reformatting or dropping keys), so the agent sees it at once. */
const shownIfChanged = (shown, text) => (typeof shown === 'string' && shown !== text ? { shown } : {});

/** Waits for autocomplete to appear, then reports whether a suggestion list is showing. */
async function suggestionsVisible(driver, view) {
  await sleep(c.SUGGESTIONS_MS);
  await driver.ctx.injectScripts(view);
  return driver.ctx.worldEval(view, s.DROPDOWN_JS, true).catch(() => false);
}

/** The handler for each server command that has one. */
const PAGE_COMMANDS = {
  ...POINTER_COMMANDS,

  /**
   * Loads a URL in the tab the command targets, after its first load and a cookie
   * pull. Only a web address: the url is the caller's, and a file: one would hand
   * them this machine's files through the next analyze or screenshot.
   */
  async navigate(driver, id, params, view) {
    if (!params?.url) return driver.ctx.sendResult(id, false, null, 'navigate needs a url. Send it again with "url".');
    if (!isWebAddress(params.url)) return driver.ctx.sendResult(id, false, null, NOT_A_WEB_ADDRESS);
    const tab = await readyTab(driver, view);
    await driver.ctx.pullCookiesFor(params.url);
    const lastErr = await loadWithRetries(tab, params.url);
    if (lastErr) return driver.ctx.sendResult(id, false, null, lastErr.message);
    await driver.ctx.injectScripts(view);
    driver.ctx.sendResult(id, true, { url: view.webContents.getURL(), title: view.webContents.getTitle() });
  },

  /** A PNG of the active tab over CDP, or a JPEG when asked (a model reads it). */
  async screenshot(driver, id, params) {
    const jpeg = params?.format === 'jpeg';
    const shot = jpeg ? { format: 'jpeg', quality: c.SCREENSHOT_JPEG_QUALITY } : { format: 'png' };
    const result = await cdp(driver.ctx.getActiveView(), 'Page.captureScreenshot', shot);
    driver.ctx.sendResult(id, true, { screenshot: `data:image/${shot.format};base64,` + result.data });
  },

  /** Clicks an element with the CDP mouse and follows any navigation it starts. */
  async click(driver, id, params) {
    const view = driver.ctx.getActiveView();
    const selector = params?.selector || '';
    const info = await driver.find(id, view, selector);
    if (!info) return;
    await cdpClick(view, info.data.x, info.data.y);
    // For iframe elements, also dispatch full pointer/mouse event sequence, CDP
    // mouse events may not trigger framework handlers (jsaction, etc.) in iframes.
    if (info.data.inIframe) await driver.ctx.worldEval(view, s.iframeClickJs(selector), true).catch(() => {});
    const { url, title } = await landedPage(driver, view);
    driver.ctx.sendResult(id, true, { clicked: true, url, title, ...withHandle(info) });
  },

  /** Clicks a field like a human, clears it, types, and reports visible suggestions. */
  async type(driver, id, params) {
    const view = driver.ctx.getActiveView();
    const selector = params?.selector || '';
    const info = await focusField(driver, id, view, selector);
    if (!info) return;
    const text = params?.text || '';
    if (!text) return driver.ctx.sendResult(id, true, { typed: true, ...withHandle(info) });
    if (await fillIfDate(driver, id, view, selector, text)) return;
    await typeAndReport(driver, id, view, { selector, text, handle: info.data.handle });
  },

  /** Presses one key where the focus is; keys that change browser state are refused. */
  async press_key(driver, id, params) {
    const view = driver.ctx.getActiveView();
    const key = params?.key || 'Enter';
    if (BLOCKED_KEYS.has(key))
      return driver.ctx.sendResult(id, false, null, `Key "${key}" is blocked, it can change browser state`);
    await cdpPressKey(view, key);
    if (key === 'Enter' && (await settleNavigation(driver, view))) await driver.ctx.injectScripts(view);
    driver.ctx.sendResult(id, true, { key });
  },

  /** Moves the CDP mouse onto an element. */
  async hover(driver, id, params) {
    const view = driver.ctx.getActiveView();
    const info = await driver.find(id, view, params?.selector || '');
    if (!info) return;
    await cdpMouseMove(view, Math.round(info.data.x), Math.round(info.data.y));
    await sleep(c.AFTER_POINTER_MS);
    driver.ctx.sendResult(id, true, { hovered: true });
  },

  /** Sets a <select>'s value. */
  async select(driver, id, params) {
    const view = driver.ctx.getActiveView();
    await driver.ctx.injectScripts(view);
    const result = await driver.ctx.worldEval(
      view,
      s.selectOptionJs(params?.selector || '', params?.value || ''),
      true,
    );
    driver.ctx.sendResult(id, result?.ok ?? true, result?.data, result?.error);
  },

  /**
   * Server-internal: the channel CAPTCHA and MFA handling use.
   *
   * Runs in the PAGE's world, not the analyzer's isolated one: clearing a
   * captcha means calling back into globals the page defined
   * (`___grecaptcha_cfg.clients[…].callback`), which an isolated world
   * cannot see. Not a public command, /browsers/:id/command rejects it,
   * and only captcha.js and mfa.js reach it.
   */
  async evaluate_raw(driver, id, params, view) {
    driver.ctx.sendResult(id, true, { result: await cdpEval(view, String(params?.expression || '')) });
  },
};

module.exports = { PAGE_COMMANDS };
