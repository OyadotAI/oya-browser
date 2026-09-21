/**
 * The pointer: clicks, hovers, drags and wheel scrolls. The vocabulary the Oya
 * client has always had, so a dashboard driving a browser never has to ask
 * which kind it is.
 */
import { elementSelector } from '../browser-scripts.ts';
import { DOUBLE_CLICK, DRAG_STEPS, SCROLL_PAGE_FRACTION, WHEEL_ORIGIN } from '../constants.ts';
import type { CDPDriver } from '../driver.ts';
import type { Handler } from './types.ts';

/** The params' x/y as numbers, 0 when missing. */
function coords(params) {
  return { x: Number(params.x) || 0, y: Number(params.y) || 0 };
}

/** Clicks an analyzer element and reports where the page ended up. */
export const click: Handler = async (driver, params) => {
  if (!params.element_id) return { ok: false, error: 'element_id required' };
  const { x, y } = await driver.locate(elementSelector(params.element_id));
  await driver.clickAt(x, y);
  return { ok: true, data: { clicked: true, url: await driver.evaluate('location.href') } };
};

/** Clicks at viewport coordinates. */
export const clickCoords: Handler = async (driver, params) => {
  const { x, y } = coords(params);
  await driver.clickAt(x, y);
  return { ok: true, data: { clicked: true } };
};

/** Moves the pointer to viewport coordinates. */
export const mouseMove: Handler = async (driver, params) => {
  const { x, y } = coords(params);
  await driver.mouse('mouseMoved', x, y);
  return { ok: true };
};

/** Where a selector or an analyzer element sits, or else the params' coordinates. */
async function targetPoint(driver: CDPDriver, params) {
  if (params.selector || params.element_id) return driver.locate(params.selector || elementSelector(params.element_id));
  return coords(params);
}

/** Double-clicks a selector, an analyzer element, or coordinates. */
export const doubleClick: Handler = async (driver, params) => {
  const { x, y } = await targetPoint(driver, params);
  await driver.mouse('mousePressed', x, y, 'left', 1);
  await driver.mouse('mouseReleased', x, y, 'left', 1);
  await driver.mouse('mousePressed', x, y, 'left', DOUBLE_CLICK);
  await driver.mouse('mouseReleased', x, y, 'left', DOUBLE_CLICK);
  return { ok: true };
};

/** Presses at one point, moves to another and releases. */
export const drag: Handler = async (driver, params) => {
  const from = { x: Number(params.from_x) || 0, y: Number(params.from_y) || 0 };
  const to = { x: Number(params.to_x) || 0, y: Number(params.to_y) || 0 };
  await driver.mouse('mouseMoved', from.x, from.y);
  await driver.mouse('mousePressed', from.x, from.y);
  await dragPath(driver, from, to);
  await driver.mouse('mouseReleased', to.x, to.y);
  return { ok: true };
};

/**
 * A few intermediate moves, or drag handlers that watch for movement
 * thresholds never fire.
 */
async function dragPath(driver: CDPDriver, from, to) {
  for (let i = 1; i <= DRAG_STEPS; i++) {
    const t = i / DRAG_STEPS;
    await driver.mouse('mouseMoved', from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
  }
}

/** Moves the pointer over an analyzer element or to coordinates. */
export const hover: Handler = async (driver, params) => {
  const { x, y } = params.element_id ? await driver.locate(elementSelector(params.element_id)) : coords(params);
  await driver.mouse('mouseMoved', x, y);
  return { ok: true };
};

/** Where the wheel event lands on one axis: the caller's point, or a spot near the corner. */
function wheelAt(v) {
  return Number.isFinite(v) ? v : WHEEL_ORIGIN;
}

/** Wheels the page by an amount, or most of a viewport; `sign` is -1 for up, 1 for down. */
async function wheelBy(driver: CDPDriver, params, sign: number) {
  const { height } = await driver.viewport();
  const deltaY = (params.amount || height * SCROLL_PAGE_FRACTION) * sign;
  const event = { type: 'mouseWheel', x: wheelAt(params.x), y: wheelAt(params.y), deltaX: 0, deltaY };
  await driver.conn.send('Input.dispatchMouseEvent', event, driver.sessionId);
  return { ok: true };
}

/** Wheels the page up. */
export const scrollUp: Handler = (driver, params) => wheelBy(driver, params, -1);
/** Wheels the page down. */
export const scrollDown: Handler = (driver, params) => wheelBy(driver, params, 1);
