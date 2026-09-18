/**
 * Server commands for the raw pointer and keyboard: coordinate clicks, mouse
 * moves, double clicks, drags, scrolls and typing without a target. Command
 * map: action → handler(driver, id, params); each answers through sendResult.
 */
const { cdp, cdpEval } = require('../cdp.cjs');
const { sleep, cdpTypeText, cdpClick, cdpMouseMove, cdpScroll } = require('../input.cjs');
const { jitter } = require('../input/timing.cjs');
const { VIEWPORT_JS, scrollResultJs } = require('./scripts.cjs');
const c = require('./constants.cjs');

/** Where a double click lands: given coordinates, or an element's centre. Null once an error is answered. */
async function doubleClickTarget(driver, id, view, params) {
  if (params?.x !== undefined && params?.y !== undefined) return { x: params.x, y: params.y };
  if (!params?.selector) {
    driver.ctx.sendResult(id, false, null, 'Provide x,y coordinates or element_id');
    return null;
  }
  const info = await driver.find(id, view, params.selector);
  return info && { x: info.data.x, y: info.data.y };
}

/** Presses the left button at (x, y) as click number `clickCount`. */
const pressLeft = (view, x, y, clickCount) =>
  cdp(view, 'Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', buttons: 1, clickCount });

/** Releases the left button at (x, y) as click number `clickCount`. */
const releaseLeft = (view, x, y, clickCount) =>
  cdp(view, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount });

/** Presses and holds the left button at `point` to start a drag. */
const holdLeft = (view, point) =>
  cdp(view, 'Input.dispatchMouseEvent', { type: 'mousePressed', x: point.x, y: point.y, button: 'left', buttons: 1 });

/** Whether a scroll comes from live control: a known pointer and no smoothing. */
const isLiveScroll = (params) => params?.smooth === false && Number.isFinite(params?.x) && Number.isFinite(params?.y);

/** Loads the analyzer and reads the viewport's size. */
async function viewportOf(driver, view) {
  await driver.ctx.injectScripts(view);
  return cdpEval(view, VIEWPORT_JS);
}

/** Moves to `from`, pauses, and presses the left button there. */
async function startDrag(view, from) {
  await cdpMouseMove(view, from.x, from.y);
  await sleep(jitter(c.BEFORE_PRESS));
  await holdLeft(view, from);
  await sleep(c.DRAG_PRESS_MS);
}

/** Two CDP clicks at (x, y), the second counted as a double click. */
async function clickTwice(view, x, y) {
  await pressLeft(view, x, y, 1);
  await releaseLeft(view, x, y, 1);
  await sleep(jitter(c.DOUBLE_CLICK_GAP));
  await pressLeft(view, x, y, c.DOUBLE_CLICK);
  await releaseLeft(view, x, y, c.DOUBLE_CLICK);
}

/** Moves the held button along a straight line from `from` to `to`. */
async function dragAlong(view, from, to) {
  for (let i = 1; i <= c.DRAG_STEPS; i++) {
    const t = i / c.DRAG_STEPS;
    const x = Math.round(from.x + (to.x - from.x) * t);
    const y = Math.round(from.y + (to.y - from.y) * t);
    await cdp(view, 'Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, button: 'left', buttons: 1 });
    await sleep(c.DRAG_STEP_MS);
  }
}

/**
 * Live control already has a pointer position and a trackpad delta.
 * Dispatch it once: synthetic smoothing and page analysis add hundreds
 * of milliseconds and cause successive gestures to overlap.
 */
async function scrollOnce(driver, id, view, params) {
  const amount = Math.max(0, Number(params.amount) || 0);
  await cdpScroll(view, params.x, params.y, 0, params.direction === 'up' ? -amount : amount);
  driver.ctx.sendResult(id, true, { direction: params.direction, amount });
}

/** Smooth scroll: `delta` from the viewport's centre, broken into wheel-notch increments. */
async function smoothScroll(view, vp, delta) {
  const cx = Math.round((vp?.w || c.FALLBACK_VIEWPORT.w) / c.HALF);
  const cy = Math.round((vp?.h || c.FALLBACK_VIEWPORT.h) / c.HALF);
  const steps = Math.max(c.MIN_SCROLL_STEPS, Math.round(Math.abs(delta) / c.SCROLL_STEP_PX));
  const stepDelta = delta / steps;
  for (let i = 0; i < steps; i++) {
    await cdpScroll(view, cx, cy, 0, stepDelta);
    await sleep(jitter(c.SCROLL_PAUSE));
  }
}

/** The handler for each pointer and raw keyboard command. */
const POINTER_COMMANDS = {
  /** A CDP click at page coordinates. */
  async click_coordinates(driver, id, params) {
    const view = driver.ctx.getActiveView();
    const x = params?.x ?? 0;
    const y = params?.y ?? 0;
    await cdpClick(view, x, y);
    await sleep(c.AFTER_POINTER_MS);
    const url = view.webContents.getURL(),
      title = view.webContents.getTitle();
    driver.ctx.sendResult(id, true, { clicked: true, x, y, url, title });
  },

  /** Moves the CDP mouse to page coordinates. */
  async mouse_move(driver, id, params) {
    const view = driver.ctx.getActiveView();
    const x = params?.x ?? 0;
    const y = params?.y ?? 0;
    await cdpMouseMove(view, x, y);
    driver.ctx.sendResult(id, true, { moved: true, x, y });
  },

  /** A double click at coordinates or on an element. */
  async double_click(driver, id, params) {
    const view = driver.ctx.getActiveView();
    const point = await doubleClickTarget(driver, id, view, params);
    if (!point) return;
    await cdpMouseMove(view, point.x, point.y);
    await sleep(jitter(c.BEFORE_PRESS));
    await clickTwice(view, point.x, point.y);
    await sleep(c.AFTER_POINTER_MS);
    driver.ctx.sendResult(id, true, { double_clicked: true, x: point.x, y: point.y });
  },

  /** Types raw text with the CDP keyboard, whatever has focus. */
  async keyboard_type(driver, id, params) {
    const view = driver.ctx.getActiveView();
    const text = params?.text || '';
    if (!text) return driver.ctx.sendResult(id, true, { typed: true });
    await cdpTypeText(view, text);
    driver.ctx.sendResult(id, true, { typed: true, text });
  },

  /** Presses at one point, moves in a straight line, releases at another. */
  async drag(driver, id, params) {
    const view = driver.ctx.getActiveView();
    const from = { x: params?.from_x ?? 0, y: params?.from_y ?? 0 };
    const to = { x: params?.to_x ?? 0, y: params?.to_y ?? 0 };
    await startDrag(view, from);
    await dragAlong(view, from, to);
    await cdp(view, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x: to.x, y: to.y, button: 'left' });
    driver.ctx.sendResult(id, true, { dragged: true, from, to });
  },

  /** Scrolls: once at a given point for live control, otherwise smoothly, then analyses the page. */
  async scroll(driver, id, params) {
    const view = driver.ctx.getActiveView();
    if (isLiveScroll(params)) return scrollOnce(driver, id, view, params);
    const vp = await viewportOf(driver, view);
    const amount = params?.amount || c.SCROLL_AMOUNT;
    await smoothScroll(view, vp, params?.direction === 'up' ? -amount : amount);
    await sleep(c.SCROLL_SETTLE_MS);
    const result = await driver.ctx.worldEval(view, scrollResultJs(params, amount), true);
    driver.ctx.sendResult(id, result?.ok ?? true, result?.data, result?.error);
  },
};

module.exports = { POINTER_COMMANDS };
