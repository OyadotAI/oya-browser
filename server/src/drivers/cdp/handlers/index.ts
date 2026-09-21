/**
 * Action → handler: everything a CDP browser can be asked to do, in this
 * driver's own spelling (normalise() maps the Oya client's onto it). An
 * unknown action is answered, not thrown.
 */
import { normalise } from '../actions.ts';
import { MIN_STEP_MS } from '../constants.ts';
import { navigate, reload, back, forward } from './navigation.ts';
import { click, clickCoords, mouseMove, doubleClick, drag, hover, scrollUp, scrollDown } from './pointer.ts';
import { keyboardType, type, pressKey } from './keyboard.ts';
import { screenshot, analyze, readPage, select, wait, scrollTop, scrollBottom, evaluateRaw, cookies } from './page.ts';
import { record } from './record.ts';
import { listTabs, newTab, switchTab, closeTab } from './tabs.ts';
import type { CDPDriver } from '../driver.ts';
import type { Handler } from './types.ts';

/** The handler for each action. */
export const HANDLERS: Record<string, Handler> = {
  navigate,
  reload,
  back,
  forward,
  screenshot,
  analyze,
  read_page: readPage,
  record,
  click,
  'click-coords': clickCoords,
  mouse_move: mouseMove,
  double_click: doubleClick,
  drag,
  keyboard_type: keyboardType,
  hover,
  type,
  'press-key': pressKey,
  'scroll-top': scrollTop,
  'scroll-bottom': scrollBottom,
  'scroll-up': scrollUp,
  'scroll-down': scrollDown,
  select,
  wait,
  'list-tabs': listTabs,
  'new-tab': newTab,
  'switch-tab': switchTab,
  'close-tab': closeTab,
  evaluate_raw: evaluateRaw,
  cookies,
};

/**
 * Same action vocabulary as the Oya client, so callers never branch on client
 * type. The two grew apart, the Oya client speaks `press_key`, `list_tabs`,
 * `open_tab` and `scroll {direction}`, this driver grew hyphenated names,
 * so both spellings are accepted and normalised here rather than in every
 * caller. The SDK, the agent tools and the MCP server all go through this.
 */
export async function dispatch(driver: CDPDriver, action, params, timeoutMs) {
  if (!driver.isAlive()) throw new Error('Browser not connected');
  ({ action, params } = normalise(action, params));
  const deadline = Date.now() + timeoutMs;
  const remaining = () => Math.max(MIN_STEP_MS, deadline - Date.now());
  const handler = Object.hasOwn(HANDLERS, action) ? HANDLERS[action] : null;
  if (!handler) return { ok: false, error: `Unsupported action for a CDP browser: ${action}` };
  return handler(driver, params, remaining);
}
