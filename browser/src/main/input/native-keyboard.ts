/** Keyboard events enter the browser's native input path, never its debugger. */
import type { KeyboardInputEvent, WebContents } from 'electron';
import { sendNativeKey } from './native-key-dispatch.ts';
import { Modifier, NATIVE_KEY_NAMES, type KeyDef } from './constants.ts';
/** The exact native capabilities keyboard input requires. */
export interface NativeKeyboardView {
  /** The page's native input surface; no debugger or protocol connection is available. */
  webContents: Pick<WebContents, 'isDestroyed' | 'sendInputEvent' | 'insertText' | 'selectAll'>;
}
/** Convert the established Oya modifier flags at the input boundary. */
function nativeModifiers(bits: number): KeyboardInputEvent['modifiers'] {
  const modifiers: KeyboardInputEvent['modifiers'] = [];
  if (bits & Modifier.CTRL) modifiers.push('control');
  if (bits & Modifier.META) modifiers.push('meta');
  if (bits & Modifier.SHIFT) modifiers.push('shift');
  if (bits & Modifier.ALT) modifiers.push('alt');
  return modifiers;
}
/** Native key names differ from DOM arrow and space names. */
function nativeKeyName(def: KeyDef): string {
  return Object.hasOwn(NATIVE_KEY_NAMES, def.key) ? NATIVE_KEY_NAMES[def.key] : def.key;
}
/** Refuse a gone renderer before dispatching input or committing composed text. */
function requireTarget(view: NativeKeyboardView): void {
  if (view.webContents.isDestroyed()) throw new Error('View is destroyed');
}
/** Native text composition handles Unicode without inventing an invalid physical key code. */
function composed(def: KeyDef): boolean {
  return Boolean(def.text && !/^[\x20-\x7e]$/.test(def.text));
}
/** Press a key, then deliver its text separately unless this is an accelerator. */
export async function nativeKeyDown(view: NativeKeyboardView, def: KeyDef, bits: number): Promise<void> {
  requireTarget(view);
  if (composed(def)) return view.webContents.insertText(def.text!);
  const modifiers = nativeModifiers(bits);
  sendNativeKey(view.webContents, { type: 'keyDown', keyCode: nativeKeyName(def), modifiers });
  const text = def.text ?? (def.key === 'Enter' ? '\r' : undefined);
  if (text && !(bits & (Modifier.CTRL | Modifier.META | Modifier.ALT)))
    sendNativeKey(view.webContents, { type: 'char', keyCode: text, modifiers });
}
/** Match physical key-down events; a composed character has no synthetic key-up. */
export function nativeKeyUp(view: NativeKeyboardView, def: KeyDef, bits: number): void {
  requireTarget(view);
  if (composed(def)) return;
  sendNativeKey(view.webContents, { type: 'keyUp', keyCode: nativeKeyName(def), modifiers: nativeModifiers(bits) });
}
