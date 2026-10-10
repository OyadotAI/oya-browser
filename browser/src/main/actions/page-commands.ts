/**
 * Server commands that act on the page's content: navigate, screenshot, and
 * element clicks, typing, keys, hovers and selects. Command map: action →
 * handler(driver, id, params, view); each answers through sendResult.
 * Pointer and raw keyboard commands are in pointer-commands.cjs.
 */
import { capturePage, evaluatePage, type NativePage as PageView } from '../native/index.ts';
import { actionNavigation } from './action-navigation.ts';
import { sleep, jitter } from '../input/timing.ts';
import * as s from './scripts.ts';
import * as c from './constants.ts';
import { POINTER_COMMANDS } from './pointer-commands.ts';
import { HISTORY_COMMANDS } from './history-commands.ts';
import * as queries from '../../page/queries.ts';
import { isDateInput, dateInputValue, unreadableDate } from '../../page/date-value.ts';
import { isWebAddress, NOT_A_WEB_ADDRESS } from '../tabs/navigation.ts';
import { loadInTab, isUnprotected } from '../tabs/load.ts';
import type { PageDriver, TabView } from './driver.ts';
import type { CommandId, CommandParams, ElementSpot, Landed, Located, ScriptResult } from './types.ts';

/** A server command: answers through sendResult, given the view it targets. */
export type PageHandler = (
  driver: PageDriver,
  id: CommandId,
  params: CommandParams | undefined,
  view: PageView,
) => Promise<void> | void;

/** Presses one key where the focus is. */
type PressKey = (view: PageView, key: string) => Promise<void>;

/** A field and the text to type into it. */
interface FieldText {
  /** The field. */
  selector: string;
  /** What to type. */
  text: string;
  /** The handles a replay finds the field by again, when the finder read them. */
  handle?: object;
}

/** A date or time field and the value it is to take. */
interface DateFill {
  /** The field. */
  selector: string;
  /** The input's type: date, time, month... */
  type: string;
  /** The value in the input's own format. */
  value: string;
}

/** Keys that zoom, open the emoji picker, or trigger OS shortcuts. */
const BLOCKED_KEYS: ReadonlySet<string> = new Set([
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
async function readyTab(driver: PageDriver, view: PageView): Promise<TabView> {
  const tab: TabView = driver.deps.tabs().find((t) => t.view === view) ?? { view };
  await driver.waitForTabReady(tab);
  return tab;
}

/** One load of `url`: null when it loaded, otherwise the error. A tab that is not protected is never retried: it throws. */
async function attemptLoad(tab: TabView, url: string): Promise<Error | null> {
  try {
    await loadInTab(tab, url);
    return null;
  } catch (navErr) {
    if (isUnprotected(navErr)) throw navErr;
    return navErr as Error;
  }
}

/** Loads `url`, retrying real failures; resolves to the last error, or null once loaded or aborted. */
async function loadWithRetries(tab: TabView, url: string): Promise<Error | null> {
  for (let attempt = 0; ; attempt++) {
    const navErr = await attemptLoad(tab, url);
    if (!navErr || navErr.message?.includes('ERR_ABORTED')) return null;
    if (attempt === c.NAVIGATE_RETRIES) return navErr;
    await sleep(c.NAVIGATE_RETRY_MS);
  }
}

/** Refuse obscured targets instead of bypassing page UI or sending input to an overlay. */
async function reachable(driver: PageDriver, id: CommandId, view: PageView, selector: string): Promise<Located | null> {
  const info = await driver.find(id, view, selector);
  if (!info?.data.covered) return info;
  driver.deps.sendResult(id, false, null, c.COVERED_TARGET_ERROR, 'element_covered');
  return null;
}

/**
 * The handles a replay finds this element by again, when the finder read them.
 * Reported from here because here the element is unambiguous, an id from an
 * earlier analysis may name nothing by the time the step is recorded.
 */
const withHandle = (info: Located): Pick<ElementSpot, 'handle'> =>
  info?.data?.handle ? { handle: info.data.handle } : {};

/** Read the page after the action-owned native navigation watcher settles. */
async function landedPage(driver: PageDriver, view: PageView): Promise<Landed> {
  const url = view.webContents.getURL();
  const title = view.webContents.getTitle();
  await driver.deps.injectScripts(view);
  return { url, title };
}

/**
 * Find element and click on it (natural focus, like a human clicking the
 * field), then pause before typing. Resolves to the element, or null once a
 * missing one is answered.
 */
async function focusField(driver: PageDriver, id: CommandId, view: PageView, selector: string) {
  const info = await reachable(driver, id, view, selector);
  if (!info) return null;
  await driver.mouse.click(view, info.data.x, info.data.y);
  await sleep(jitter(c.FOCUS_PAUSE));
  return info;
}

/** The field's current text, or null when it cannot be read. */
const fieldValue = (driver: PageDriver, view: PageView, selector: string): Promise<unknown> =>
  driver.deps.worldEval(view, s.fieldValueJs(selector)).catch(() => null);

/** Whether a field holds text to clear. An unfilled mask such as __/__/____ holds none. */
const hasContent = (value: unknown): boolean => typeof value !== 'string' || /[\p{L}\p{N}]/u.test(value);

/**
 * Empties a field before typing. A filled one is selected and deleted with a
 * real Backspace (`press` sends it to the right frame), then given a moment:
 * masked fields redraw after a clear and drop keys typed meanwhile. An unfilled
 * mask is left alone, with the caret moved to its start, where the first
 * character belongs: a click in its middle leaves the caret there.
 */
async function clearField(driver: PageDriver, view: PageView, selector: string, press: PressKey): Promise<unknown> {
  if (!hasContent(await fieldValue(driver, view, selector)))
    return driver.deps.worldEval(view, s.caretStartJs(selector)).catch(() => {});
  const selected = await driver.deps.worldEval(view, s.selectFieldJs(selector)).catch(() => false);
  if (selected !== 'select') return;
  await sleep(jitter(c.CLEAR_PAUSE));
  await press(view, 'Backspace');
  await sleep(c.CLEAR_SETTLE_MS);
}

/** Selects only the intended field, then clears and types with native keyboard input. */
async function typeIntoField(driver: PageDriver, view: PageView, selector: string, text: string): Promise<void> {
  await clearField(driver, view, selector, (v, key) => driver.keyboard.press(v, key));
  await driver.keyboard.type(view, text);
}

/**
 * A native date or time input gets its value set, not typed: typed digits land
 * in whichever segment has focus. Answers false for any other field.
 */
async function fillIfDate(driver: PageDriver, id: CommandId, view: PageView, field: FieldText): Promise<boolean> {
  const { selector, text } = field;
  const type = await driver.deps.worldEval<string | null>(view, s.inputTypeJs(selector)).catch(() => null);
  if (!type || !isDateInput(type)) return false;
  const value = dateInputValue(type, text);
  if (value === null) driver.deps.sendResult(id, false, null, unreadableDate(type, text));
  else await setDate(driver, id, view, { selector, type, value });
  return true;
}

/** Sets the value and reports it, or says the field refused it (outside its min or max, say). */
async function setDate(driver: PageDriver, id: CommandId, view: PageView, { selector, type, value }: DateFill) {
  const kept = await driver.deps.worldEval(view, s.setInputValueJs(selector, value)).catch(() => null);
  if (kept === value) return driver.deps.sendResult(id, true, { typed: true, value });
  driver.deps.sendResult(id, false, null, `The ${type} field did not accept ${value}`);
}

/** Types into a text field, then reports whether suggestions appeared. */
async function typeAndReport(driver: PageDriver, id: CommandId, view: PageView, field: FieldText): Promise<void> {
  const { selector, text, handle } = field;
  await typeIntoField(driver, view, selector, text);
  const suggestions = await suggestionsVisible(driver, view);
  const extra = { ...(handle ? { handle } : {}), ...shownIfChanged(await fieldValue(driver, view, selector), text) };
  driver.deps.sendResult(id, true, { typed: true, suggestions_visible: suggestions, ...extra });
}

/** What the field shows when it is not what was typed (a mask reformatting or dropping keys), so the agent sees it at once. */
const shownIfChanged = (shown: unknown, text: string): Record<string, string> =>
  typeof shown === 'string' && shown !== text ? { shown } : {};

/** Waits for autocomplete to appear, then reports whether a suggestion list is showing. */
async function suggestionsVisible(driver: PageDriver, view: PageView): Promise<unknown> {
  await sleep(c.SUGGESTIONS_MS);
  await driver.deps.injectScripts(view);
  return driver.deps.worldEval(view, s.DROPDOWN_JS).catch(() => false);
}

/** The handler for each server command that has one. */
export const PAGE_COMMANDS: Readonly<Record<string, PageHandler>> = {
  ...POINTER_COMMANDS,
  ...HISTORY_COMMANDS,

  /**
   * Loads a URL in the tab the command targets, after its first load and a cookie
   * pull. Only a web address: the url is the caller's, and a file: one would hand
   * them this machine's files through the next analyze or screenshot.
   */
  async navigate(driver, id, params, view) {
    if (!params?.url) return driver.deps.sendResult(id, false, null, 'navigate needs a url. Send it again with "url".');
    if (!isWebAddress(params.url)) return driver.deps.sendResult(id, false, null, NOT_A_WEB_ADDRESS);
    const tab = await readyTab(driver, view);
    await driver.deps.pullCookiesFor(params.url);
    const lastErr = await loadWithRetries(tab, params.url);
    if (lastErr) return driver.deps.sendResult(id, false, null, lastErr.message);
    await driver.deps.injectScripts(view);
    driver.deps.sendResult(id, true, { url: view.webContents.getURL(), title: view.webContents.getTitle() });
  },

  /** Capture the command's exact native view, or a JPEG when asked (a model reads it). */
  async screenshot(driver, id, params, view) {
    const jpeg = params?.format === 'jpeg';
    const screenshot = await capturePage(view, jpeg ? c.SCREENSHOT_JPEG_QUALITY : undefined);
    driver.deps.sendResult(id, true, { screenshot });
  },

  /** Clicks an unobscured element with native input and follows any navigation it starts. */
  async click(driver, id, params) {
    const view = driver.activeView();
    const selector = params?.selector || '';
    const info = await reachable(driver, id, view, selector);
    if (!info) return;
    await actionNavigation(view, () => driver.mouse.click(view, info.data.x, info.data.y));
    const { url, title } = await landedPage(driver, view);
    driver.deps.sendResult(id, true, { clicked: true, url, title, ...withHandle(info) });
  },

  /** Clicks a field like a human, clears it, types, and reports visible suggestions. */
  async type(driver, id, params) {
    const view = driver.activeView();
    const selector = params?.selector || '';
    const info = await focusField(driver, id, view, selector);
    if (!info) return;
    const text = params?.text || '';
    if (!text) return driver.deps.sendResult(id, true, { typed: true, ...withHandle(info) });
    if (await fillIfDate(driver, id, view, { selector, text })) return;
    await typeAndReport(driver, id, view, { selector, text, handle: info.data.handle });
  },

  /** Presses one key where the focus is; keys that change browser state are refused. */
  async press_key(driver, id, params) {
    const view = driver.activeView();
    const key = params?.key || 'Enter';
    if (BLOCKED_KEYS.has(key))
      return driver.deps.sendResult(id, false, null, `Key "${key}" is blocked, it can change browser state`);
    await pressAndSettle(driver, view, key);
    driver.deps.sendResult(id, true, { key });
  },

  /** Moves the native pointer onto an element. */
  async hover(driver, id, params) {
    const view = driver.activeView();
    const info = await driver.find(id, view, params?.selector || '');
    if (!info) return;
    await driver.mouse.move(view, Math.round(info.data.x), Math.round(info.data.y));
    await sleep(c.AFTER_POINTER_MS);
    driver.deps.sendResult(id, true, { hovered: true });
  },

  /** Sets a <select>'s value. */
  async select(driver, id, params) {
    const view = driver.activeView();
    await driver.deps.injectScripts(view);
    const result = await driver.deps.worldEval<ScriptResult | null>(
      view,
      s.selectOptionJs(params?.selector || '', params?.value || ''),
    );
    driver.deps.sendResult(id, result?.ok ?? true, result?.data, result?.error);
  },

  /**
   * The agent's script, in the analyzer's isolated world, so the page never sees
   * it (page-queries.cjs runScriptJs). Server-internal: the agent's run_script
   * tool refuses a script that writes, and /browsers/:id/command rejects the
   * action, so no caller can send unchecked JavaScript this way.
   */
  async run_script(driver, id, params, view) {
    await driver.deps.injectScripts(view);
    driver.deps.sendResult(id, true, await driver.deps.worldEval(view, queries.runScriptJs(params?.script)));
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
    driver.deps.sendResult(id, true, { result: await evaluatePage(view, String(params?.expression || '')) });
  },
};

/** Only Enter can initiate a document wait; other keys preserve their immediate native dispatch contract. */
async function pressAndSettle(driver: PageDriver, view: PageView, key: string): Promise<void> {
  if (key !== 'Enter') return driver.keyboard.press(view, key);
  if (await actionNavigation(view, () => driver.keyboard.press(view, key))) await driver.deps.injectScripts(view);
}
