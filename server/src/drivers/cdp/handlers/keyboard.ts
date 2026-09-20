/**
 * The keyboard: typing into an element or whatever has focus, and named keys.
 */
import { elementSelector } from '../browser-scripts.ts';
import { SELECT_CONTENTS_JS, INPUT_TYPE_JS, SET_DATE_VALUE_JS } from '../page-scripts.ts';
import dates from '../../../../../browser/scripts/date-value.cjs';
import type { CDPDriver } from '../driver.ts';
import type { Handler } from './types.ts';

/** Keys that have no text of their own, as CDP key events describe them. */
const KEY_CODES = {
  Enter: { key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, text: '\r' },
  Tab: { key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9 },
  Backspace: { key: 'Backspace', code: 'Backspace', windowsVirtualKeyCode: 8 },
  Delete: { key: 'Delete', code: 'Delete', windowsVirtualKeyCode: 46 },
  Escape: { key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 },
  ArrowUp: { key: 'ArrowUp', code: 'ArrowUp', windowsVirtualKeyCode: 38 },
  ArrowDown: { key: 'ArrowDown', code: 'ArrowDown', windowsVirtualKeyCode: 40 },
  ArrowLeft: { key: 'ArrowLeft', code: 'ArrowLeft', windowsVirtualKeyCode: 37 },
  ArrowRight: { key: 'ArrowRight', code: 'ArrowRight', windowsVirtualKeyCode: 39 },
};

/** Inserts text at the focus, as an IME would. */
function insertText(driver: CDPDriver, text: string) {
  return driver.conn.send('Input.insertText', { text }, driver.sessionId);
}

/** Types into whatever is focused, like a person typing. */
export const keyboardType: Handler = async (driver, params) => {
  await insertText(driver, String(params.text || ''));
  return { ok: true };
};

/** Types into an element (replacing its contents) when one is named, otherwise into the focus. */
export const type: Handler = async (driver, params) => {
  const text = String(params.text ?? '');
  if (params.element_id == null && !params.selector) return insertText(driver, text).then(() => ({ ok: true }));
  const selector = params.selector || elementSelector(params.element_id);
  const { x, y } = await driver.locate(selector);
  await driver.clickAt(x, y);
  const inputType = await driver.evaluate(INPUT_TYPE_JS(selector));
  if (text && dates.isDateInput(inputType)) return fillDate(driver, selector, inputType, text);
  return replaceContents(driver, selector, text);
};

/** Selects the element's contents and types over them. */
async function replaceContents(driver: CDPDriver, selector: string, text: string) {
  await driver.evaluate(SELECT_CONTENTS_JS(selector));
  await insertText(driver, text);
  return { ok: true };
}

/**
 * A native date or time input gets its value set, not typed: typed digits land
 * in whichever of its locale-ordered segments has focus.
 */
async function fillDate(driver: CDPDriver, selector: string, type: string, text: string) {
  const value = dates.dateInputValue(type, text);
  if (value === null) return { ok: false, error: dates.unreadableDate(type, text) };
  const kept = await driver.evaluate(SET_DATE_VALUE_JS(selector, value));
  return kept === value ? { ok: true, value } : { ok: false, error: `The ${type} field did not accept ${value}` };
}

/** Presses a named key, or types a single character. */
export const pressKey: Handler = async (driver, params) => {
  const spec = KEY_CODES[params.key];
  if (!spec) return typeCharacter(driver, params.key);
  const keyEvent = (type) => driver.conn.send('Input.dispatchKeyEvent', { type, ...spec }, driver.sessionId);
  await keyEvent('keyDown');
  if (spec.text) await keyEvent('char');
  await keyEvent('keyUp');
  return { ok: true };
};

/**
 * A single printable character is a keypress too; the table only lists the
 * keys that have no text of their own.
 */
async function typeCharacter(driver: CDPDriver, rawKey) {
  const key = String(rawKey ?? '');
  if ([...key].length !== 1) return { ok: false, error: `Unsupported key: ${rawKey}` };
  await insertText(driver, key);
  return { ok: true };
}
