/**
 * The dev panel's quick actions. Command map: action → handler(driver, view,
 * params), each answering `{ ok, data?, error? }` by return value. Elements are
 * addressed by the analyzer's `data-ac-id`.
 */
import { capturePage, evaluatePage, type NativePage as PageView } from '../native/index.ts';
import type { Point } from '../input/mouse.ts';
import { sleep } from '../input/timing.ts';
import { VIEWPORT_JS, DEV_ANALYZE_JS, devWaitJs } from './scripts.ts';
import { HOME_URL } from '../tabs/constants.ts';
import { renderedAnalysis } from './page-format.ts';
import * as c from './constants.ts';
import type { DevAnswer, PageDriver } from './driver.ts';
import type { CommandParams, Viewport } from './types.ts';

/** A dev panel action: answers `{ ok, data?, error? }` by return value. */
type DevHandler = (driver: PageDriver, view: PageView, params?: CommandParams) => Promise<DevAnswer>;

/**
 * Actions that run without human control: they only look. Analyze is not one:
 * it renumbers the page's element ids, which would break the agent's next click.
 */
export const UNGUARDED_DEV_COMMANDS: readonly string[] = ['screenshot', 'list-tabs'];

/** The analyzer's selector for an element id. */
const byAcId = (elementId: unknown): string => `[data-ac-id="${elementId}"]`;

/** Why `params` has no usable element number, or null when it has one. */
function elementIdProblem(params: CommandParams | undefined): string | null {
  if (!params?.element_id) return 'element_id required';
  return /^\d+$/.test(String(params.element_id)) ? null : 'element_id must be a number';
}

/** Whether `value` is a real number: an empty field is not 0. */
const isCoordinate = (value: unknown): boolean => value !== '' && value !== null && Number.isFinite(Number(value));

/** Clicks into the field at `point`, then empties it. */
async function focusAndClear(driver: PageDriver, view: PageView, point: Point): Promise<void> {
  await driver.mouse.click(view, point.x, point.y);
  await sleep(c.DEV_FOCUS_MS);
  await driver.keyboard.clear(view);
}

/** One wheel event of `delta` at the viewport's centre. */
async function devScroll(driver: PageDriver, view: PageView, delta: number): Promise<DevAnswer> {
  const vp = await evaluatePage<Viewport | null>(view, VIEWPORT_JS);
  const x = (vp?.w || c.FALLBACK_VIEWPORT.w) / c.HALF;
  await driver.mouse.scroll(view, x, (vp?.h || c.FALLBACK_VIEWPORT.h) / c.HALF, 0, delta);
  return { ok: true };
}

/** The handler for each dev panel action. */
export const DEV_COMMANDS: Readonly<Record<string, DevHandler>> = {
  /** The analyzer's full read of the page. */
  async analyze(driver, view) {
    await driver.deps.injectScripts(view);
    return renderedAnalysis(driver.deps, await driver.deps.worldEval(view, DEV_ANALYZE_JS));
  },

  /** A PNG of the active tab. */
  async screenshot(driver, view) {
    return { ok: true, data: { screenshot: await capturePage(view) } };
  },

  /** Scrolls down by `amount` pixels. */
  'scroll-down': (driver, view, params) => devScroll(driver, view, (params?.amount || c.DEV_SCROLL_AMOUNT) as number),

  /** Scrolls up by `amount` pixels. */
  'scroll-up': (driver, view, params) => devScroll(driver, view, -Number(params?.amount || c.DEV_SCROLL_AMOUNT)),

  /** Reloads the page. */
  async reload(driver, view) {
    view.webContents.reload();
    return { ok: true };
  },

  /** Goes to an address the way the address bar does (its search, cookies and recording included). */
  async navigate(driver, view, params) {
    if (!params?.url) return { ok: false, error: 'URL required' };
    await driver.deps.navigate(params.url);
    return { ok: true, data: { url: view.webContents.getURL(), title: view.webContents.getTitle() } };
  },

  /** Clicks an element by id with native pointer input. */
  async click(driver, view, params) {
    const problem = elementIdProblem(params);
    if (problem) return { ok: false, error: problem };
    const info = await driver.locate(view, byAcId(params?.element_id));
    if (!info?.ok) return { ok: false, error: info?.error || 'Element not found' };
    await driver.mouse.click(view, info.data.x, info.data.y);
    await sleep(c.NAVIGATION_START_MS);
    return { ok: true, data: { clicked: true, url: view.webContents.getURL() } };
  },

  /** Clicks a field by id, clears it and types into it. */
  async type(driver, view, params) {
    const problem = params?.text ? elementIdProblem(params) : 'element_id and text required';
    if (problem) return { ok: false, error: problem };
    const info = await driver.locate(view, byAcId(params?.element_id));
    if (!info?.ok) return { ok: false, error: info?.error || 'Element not found' };
    await focusAndClear(driver, view, info.data);
    await driver.keyboard.type(view, String(params?.text));
    return { ok: true, data: { typed: true } };
  },

  /** Presses one key with the CDP keyboard. */
  async 'press-key'(driver, view, params) {
    if (!params?.key) return { ok: false, error: 'key required' };
    await driver.keyboard.press(view, params.key);
    return { ok: true, data: { key: params.key } };
  },

  /** Moves the mouse onto an element by id. */
  async hover(driver, view, params) {
    const problem = elementIdProblem(params);
    if (problem) return { ok: false, error: problem };
    const info = await driver.locate(view, byAcId(params?.element_id));
    if (!info?.ok) return { ok: false, error: info?.error || 'Element not found' };
    await driver.mouse.move(view, Math.round(info.data.x), Math.round(info.data.y));
    return { ok: true, data: { hovered: true } };
  },

  /** A CDP click at page coordinates. */
  async 'click-coords'(driver, view, params) {
    if (!isCoordinate(params?.x) || !isCoordinate(params?.y)) return { ok: false, error: 'x and y required' };
    const [x, y] = [Number(params?.x), Number(params?.y)];
    await driver.mouse.click(view, x, y);
    return { ok: true, data: { clicked: true, x, y } };
  },

  /** Waits for a CSS selector to match. */
  async wait(driver, view, params) {
    if (!params?.selector) return { ok: false, error: 'selector required' };
    await driver.deps.injectScripts(view);
    return await driver.deps.worldEval<DevAnswer>(view, devWaitJs(params));
  },

  /** The open tabs. */
  async 'list-tabs'(driver) {
    const { deps } = driver;
    const tabs = deps
      .tabs()
      .map((t) => ({ id: t.id, title: t.title, url: t.url, active: t.id === deps.activeTabId() }));
    return { ok: true, data: { tabs } };
  },

  /** Opens a tab on the home page, like the new tab button. */
  async 'new-tab'(driver, view, params) {
    const tabId = driver.deps.createTab(params?.url || HOME_URL, true);
    return { ok: true, data: { tab_id: tabId } };
  },
};
