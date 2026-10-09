/** Native physical key dispatch preserves Oya shortcut provenance and rejects unsupported physical metadata. */
import type { KeyboardInputEvent } from 'electron';
import type { NativePage } from './page.ts';
import { sendNativeKey, keyDef, nativeKeyName } from '../input/index.ts';
import { inputModifiers } from './pointer-input.ts';
/** Supported navigation/editing keys have stable native accelerator meanings. */
const KEYS = new Set([
  'Enter',
  'Tab',
  'Backspace',
  'Delete',
  'Escape',
  'ArrowUp',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
  'Home',
  'End',
  'PageUp',
  'PageDown',
  ' ',
]);
/** Keyboard event kinds with actual native equivalents. */
const TYPES = new Set(['keyDown', 'rawKeyDown', 'keyUp', 'char']);
/** Already parsed, but still untrusted, compatibility request. */
type Params = Record<string, unknown>;
/** Validate everything before dispatch: unsupported key layouts never become a different physical key. */
export function validateKey(params: Params): void {
  if (typeof params.type !== 'string' || !TYPES.has(params.type)) throw Error('Unsupported native key event type');
  inputModifiers(params.modifiers);
  const def = definition(params);
  if (params.code !== undefined && params.code !== def.code) throw Error('Unsupported native physical key code');
  if (params.windowsVirtualKeyCode !== undefined && params.windowsVirtualKeyCode !== def.keyCode)
    throw Error('Native virtual key code does not match the key');
  validateText(params);
}
/** Unicode composition is a separate acknowledged Input.insertText operation, not a forged physical key. */
function definition(params: Params) {
  if (params.type === 'char' && params.key === undefined && typeof params.text === 'string')
    return logicalDefinition(params.text);
  const key = params.key;
  if (typeof key !== 'string' || (!KEYS.has(key) && !/^[a-zA-Z0-9]$/.test(key)))
    throw Error('Unsupported native key; use Input.insertText for composed text');
  return logicalDefinition(key);
}
/** Raw key/up events cannot silently discard an explicitly requested text commit. */
function validateText(params: Params): void {
  if (params.type === 'char' && params.text === undefined) throw Error('Character events require text');
  if (params.text === undefined) return;
  if (typeof params.text !== 'string' || !/^[\x20-\x7e\r]$/.test(params.text))
    throw Error('Use Input.insertText for Unicode or multi-character text');
  if (!['keyDown', 'char'].includes(String(params.type))) throw Error('Text requires keyDown or char');
}
/** Exact target dispatch never activates another tab or exposes native shortcut privileges to page script. */
export function dispatchNativeKey(page: NativePage, params: Params): object {
  validateKey(params);
  if (page.webContents.isDestroyed()) throw Error('View is destroyed');
  const modifiers = inputModifiers(params.modifiers);
  const keyCode = params.type === 'char' ? String(params.text) : nativeKeyName(definition(params));
  sendNativeKey(page.webContents, { type: params.type as KeyboardInputEvent['type'], keyCode, modifiers });
  if (params.type === 'keyDown' && params.text !== undefined)
    sendNativeKey(page.webContents, { type: 'char', keyCode: String(params.text), modifiers });
  return {};
}

/** Space and carriage return have named physical metadata rather than invented zero key codes. */
function logicalDefinition(key: string) {
  const names: Record<string, string> = { ' ': 'Space', '\r': 'Enter' };
  return keyDef(Object.hasOwn(names, key) ? names[key] : key);
}
