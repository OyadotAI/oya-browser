/**
 * Server commands for the raw pointer and keyboard: coordinate clicks, mouse
 * moves, double clicks, drags, scrolls and typing without a target. Command
 * map: action → handler(driver, id, params); each answers through sendResult.
 */
import { nativePointer, nativeDrag } from '../input/index.ts';
import type { TabView } from '../tabs/types.ts';
import type { Mouse } from '../input/mouse.ts';
import { sleep, jitter } from '../input/timing.ts';
import { VIEWPORT_JS, scrollResultJs } from './scripts.ts';
import { renderedAnalysis } from './page-format.ts';
import * as c from './constants.ts';
import type { PageDriver } from './driver.ts';
import type { PageHandler } from './page-commands.ts';
import type { CommandId, CommandParams, Viewport } from './types.ts';

/** Existing native page surface; the command boundary retains authorization. */
type PageView = TabView;

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
const pressLeft = (view: PageView, x: number, y: number, clickCount: number): void =>
  nativePointer(view, { type: 'mouseDown', x, y, button: 'left', clickCount });

/** Releases the left button at (x, y) as click number `clickCount`. */
const releaseLeft = (view: PageView, x: number, y: number, clickCount: number): void =>
  nativePointer(view, { type: 'mouseUp', x, y, button: 'left', clickCount });

/** Whether a scroll comes from live control: a known pointer and no smoothing. */
const isLiveScroll = (params: CommandParams | undefined): params is LiveScroll =>
  params?.smooth === false && Number.isFinite(params?.x) && Number.isFinite(params?.y);

/** Loads the analyzer and reads the viewport's size. */
async function viewportOf(driver: PageDriver, view: PageView): Promise<Viewport | null> {
  await driver.deps.injectScripts(view);
  return view.webContents.executeJavaScript(VIEWPORT_JS);
}

/** Two native clicks at (x, y), the second counted as a double click. */
async function clickTwice(view: PageView, x: number, y: number): Promise<void> {
  await pressLeft(view, x, y, 1);
  await releaseLeft(view, x, y, 1);
  await sleep(jitter(c.DOUBLE_CLICK_GAP));
  await pressLeft(view, x, y, c.DOUBLE_CLICK);
  await releaseLeft(view, x, y, c.DOUBLE_CLICK);
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
  /** A native click at page coordinates. */
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

  /** Moves the native mouse to page coordinates. */
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

  /** Types raw text with the keyboard, whatever has focus. */
  async keyboard_type(driver, id, params) {
    const view = driver.activeView();
    const text = params?.text || '';
    if (!text) return driver.deps.sendResult(id, true, { typed: true });
    await driver.keyboard.type(view, text);
    driver.deps.sendResult(id, true, { typed: true, text });
  },

  /** Run one acknowledged native gesture without handing HTML drags to an uncontrolled OS session. */
  async drag(driver, id, params) {
    const view = driver.activeView();
    const from = { x: params?.from_x ?? 0, y: params?.from_y ?? 0 };
    const to = { x: params?.to_x ?? 0, y: params?.to_y ?? 0 };
    const kind = await nativeDrag(view, from, to);
    driver.deps.sendResult(id, true, { dragged: true, kind, from, to });
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
