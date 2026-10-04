/**
 * Server commands for the raw pointer and keyboard: coordinate clicks, mouse
 * moves, double clicks, drags, scrolls and typing without a target. Command
 * map: action → handler(driver, id, params); each answers through sendResult.
 */
import { cdp, cdpEval, type PageView } from '../cdp/cdp.ts';
import type { Mouse, Point } from '../input/mouse.ts';
import { sleep, jitter } from '../input/timing.ts';
import { VIEWPORT_JS, scrollResultJs } from './scripts.ts';
import { renderedAnalysis } from './page-format.ts';
import * as c from './constants.ts';
import type { PageDriver } from './driver.ts';
import type { PageHandler } from './page-commands.ts';
import type { CommandId, CommandParams, Viewport } from './types.ts';

/** A scroll from live control, which knows where the pointer is. */
interface LiveScroll extends CommandParams {
  /** The pointer's distance from the left. */
  x: number;
  /** The pointer's distance from the top. */
  y: number;
}

/** Where a double click lands: given coordinates, or an element's centre. Null once an error is answered. */
async function doubleClickTarget(driver: PageDriver, id: CommandId, view: PageView, params?: CommandParams) {
  if (params?.x !== undefined && params?.y !== undefined) return { x: params.x, y: params.y };
  if (!params?.selector) return noTarget(driver, id);
  const info = await driver.find(id, view, params.selector);
  return info && { x: info.data.x, y: info.data.y };
}

/** Tells the caller a double click needs a point or an element; there is no target. */
function noTarget(driver: PageDriver, id: CommandId): null {
  driver.deps.sendResult(id, false, null, 'Provide x,y coordinates or element_id');
  return null;
}

/** Presses the left button at (x, y) as click number `clickCount`. */
const pressLeft = (view: PageView, x: number, y: number, clickCount: number): Promise<unknown> =>
  cdp(view, 'Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', buttons: 1, clickCount });

/** Releases the left button at (x, y) as click number `clickCount`. */
const releaseLeft = (view: PageView, x: number, y: number, clickCount: number): Promise<unknown> =>
  cdp(view, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount });

/** Presses and holds the left button at `point` to start a drag. */
const holdLeft = (view: PageView, point: Point): Promise<unknown> =>
  cdp(view, 'Input.dispatchMouseEvent', { type: 'mousePressed', x: point.x, y: point.y, button: 'left', buttons: 1 });

/** Whether a scroll comes from live control: a known pointer and no smoothing. */
const isLiveScroll = (params: CommandParams | undefined): params is LiveScroll =>
  params?.smooth === false && Number.isFinite(params?.x) && Number.isFinite(params?.y);

/** Loads the analyzer and reads the viewport's size. */
async function viewportOf(driver: PageDriver, view: PageView): Promise<Viewport | null> {
  await driver.deps.injectScripts(view);
  return cdpEval<Viewport | null>(view, VIEWPORT_JS);
}

/** Moves to `from`, pauses, and presses the left button there. */
async function startDrag(mouse: Mouse, view: PageView, from: Point): Promise<void> {
  await mouse.move(view, from.x, from.y);
  await sleep(jitter(c.BEFORE_PRESS));
  await holdLeft(view, from);
  await sleep(c.DRAG_PRESS_MS);
}

/** Two CDP clicks at (x, y), the second counted as a double click. */
async function clickTwice(view: PageView, x: number, y: number): Promise<void> {
  await pressLeft(view, x, y, 1);
  await releaseLeft(view, x, y, 1);
  await sleep(jitter(c.DOUBLE_CLICK_GAP));
  await pressLeft(view, x, y, c.DOUBLE_CLICK);
  await releaseLeft(view, x, y, c.DOUBLE_CLICK);
}

/** Moves the held button along a straight line from `from` to `to`. */
async function dragAlong(view: PageView, from: Point, to: Point): Promise<void> {
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
async function scrollOnce(driver: PageDriver, id: CommandId, view: PageView, params: LiveScroll): Promise<void> {
  const amount = Math.max(0, Number(params.amount) || 0);
  const delta = params.direction === 'up' ? -amount : amount;
  await driver.mouse.scroll(view, params.x, params.y, 0, delta);
  driver.deps.sendResult(id, true, { direction: params.direction, amount });
}

/**
 * How far a smooth scroll moves: the caller's amount when it is a positive
 * number, capped, else the default. It sets how long the scroll runs and is
 * written into the result script, so the caller's own text never gets that far.
 */
function smoothAmount(params: CommandParams | undefined): number {
  const amount = Number(params?.amount);
  return amount > 0 ? Math.min(amount, c.MAX_SCROLL_AMOUNT) : c.SCROLL_AMOUNT;
}

/** Smooth scroll: `delta` from the viewport's centre, broken into wheel-notch increments. */
async function smoothScroll(mouse: Mouse, view: PageView, vp: Viewport | null, delta: number): Promise<void> {
  const cx = Math.round((vp?.w || c.FALLBACK_VIEWPORT.w) / c.HALF);
  const cy = Math.round((vp?.h || c.FALLBACK_VIEWPORT.h) / c.HALF);
  const steps = Math.max(c.MIN_SCROLL_STEPS, Math.round(Math.abs(delta) / c.SCROLL_STEP_PX));
  const stepDelta = delta / steps;
  for (let i = 0; i < steps; i++) {
    await mouse.scroll(view, cx, cy, 0, stepDelta);
    await sleep(jitter(c.SCROLL_PAUSE));
  }
}

/** Answers a scroll with the page it landed on, analysed and rendered in the format that applies. */
async function sendAnalysis(driver: PageDriver, id: CommandId, view: PageView, params?: CommandParams, amount = 0) {
  const raw = await driver.deps.worldEval(view, scrollResultJs(params, amount));
  const result = renderedAnalysis(driver.deps, raw, params);
  driver.deps.sendResult(id, result?.ok ?? true, result?.data, result?.error);
}

/** The handler for each pointer and raw keyboard command. */
export const POINTER_COMMANDS: Readonly<Record<string, PageHandler>> = {
  /** A CDP click at page coordinates. */
  async click_coordinates(driver, id, params) {
    const view = driver.activeView();
    const x = params?.x ?? 0;
    const y = params?.y ?? 0;
    await driver.mouse.click(view, x, y);
    await sleep(c.AFTER_POINTER_MS);
    const url = view.webContents.getURL(),
      title = view.webContents.getTitle();
    driver.deps.sendResult(id, true, { clicked: true, x, y, url, title });
  },

  /** Moves the CDP mouse to page coordinates. */
  async mouse_move(driver, id, params) {
    const view = driver.activeView();
    const x = params?.x ?? 0;
    const y = params?.y ?? 0;
    await driver.mouse.move(view, x, y);
    driver.deps.sendResult(id, true, { moved: true, x, y });
  },

  /** A double click at coordinates or on an element. */
  async double_click(driver, id, params) {
    const view = driver.activeView();
    const point = await doubleClickTarget(driver, id, view, params);
    if (!point) return;
    await driver.mouse.move(view, point.x, point.y);
    await sleep(jitter(c.BEFORE_PRESS));
    await clickTwice(view, point.x, point.y);
    await sleep(c.AFTER_POINTER_MS);
    driver.deps.sendResult(id, true, { double_clicked: true, x: point.x, y: point.y });
  },

  /** Types raw text with the CDP keyboard, whatever has focus. */
  async keyboard_type(driver, id, params) {
    const view = driver.activeView();
    const text = params?.text || '';
    if (!text) return driver.deps.sendResult(id, true, { typed: true });
    await driver.keyboard.type(view, text);
    driver.deps.sendResult(id, true, { typed: true, text });
  },

  /** Presses at one point, moves in a straight line, releases at another. */
  async drag(driver, id, params) {
    const view = driver.activeView();
    const from = { x: params?.from_x ?? 0, y: params?.from_y ?? 0 };
    const to = { x: params?.to_x ?? 0, y: params?.to_y ?? 0 };
    await startDrag(driver.mouse, view, from);
    await dragAlong(view, from, to);
    await cdp(view, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x: to.x, y: to.y, button: 'left' });
    driver.deps.sendResult(id, true, { dragged: true, from, to });
  },

  /** Scrolls: once at a given point for live control, otherwise smoothly, then analyses the page. */
  async scroll(driver, id, params) {
    const view = driver.activeView();
    if (isLiveScroll(params)) return scrollOnce(driver, id, view, params);
    const vp = await viewportOf(driver, view);
    const amount = smoothAmount(params);
    await smoothScroll(driver.mouse, view, vp, params?.direction === 'up' ? -amount : amount);
    await sleep(c.SCROLL_SETTLE_MS);
    await sendAnalysis(driver, id, view, params, amount);
  },
};
