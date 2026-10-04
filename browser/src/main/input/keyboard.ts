/** Keyboard input over CDP: key definitions, presses, and typing with a human cadence. */
import { cdp, type PageView } from '../cdp/cdp.ts';
import { KEY_DEFS, Modifier, KEY_HOLD, CLEAR_PAUSE, type KeyDef } from './constants.ts';
import { sleep, jitter, typingDelay } from './timing.ts';

/** The CDP key event fields for a named key or a single character. */
export function keyDef(ch: string): KeyDef {
  // A line break is the Enter key, not a character with no key code behind it: a
  // textarea takes the break either way, but a form watching for Enter sees nothing.
  if (ch === '\n') return { ...KEY_DEFS.Enter };
  if (Object.hasOwn(KEY_DEFS, ch)) return { ...KEY_DEFS[ch] };
  const upper = ch.toUpperCase();
  const isLetter = /^[a-zA-Z]$/.test(ch);
  const isDigit = /^[0-9]$/.test(ch);
  const code = isLetter ? 'Key' + upper : isDigit ? 'Digit' + ch : '';
  const keyCode = isLetter ? upper.charCodeAt(0) : isDigit ? ch.charCodeAt(0) : 0;
  return { key: ch, code, keyCode, text: ch, shift: ch !== ch.toLowerCase() && ch === upper };
}

/** The fields a key's down and up events share. */
const keyFields = (def: KeyDef, modifiers: number) => ({
  modifiers,
  windowsVirtualKeyCode: def.keyCode,
  nativeVirtualKeyCode: def.keyCode,
  key: def.key,
  code: def.code,
});

/** Types and presses keys on a view, as a person at a keyboard would. */
export class Keyboard {
  /** The OS, which picks the select-all modifier. */
  private readonly platform: NodeJS.Platform;

  /** `platform` is the OS the shortcuts follow (process.platform). */
  constructor(platform: NodeJS.Platform) {
    this.platform = platform;
  }

  /**
   * Sends a key down. `keyDown` for every key, text or not: a `rawKeyDown`, which is
   * what a raw key event is called and what other drivers send for a key with no text,
   * arrives through Electron's debugger without ever becoming a DOM keydown. A page
   * that echoes `$(document).keydown` showed nothing for ArrowLeft, Tab, PageDown or
   * Escape while a typed character came straight back. Chrome raises no keypress for a
   * key carrying no text, so nothing is gained by the raw form and a key press is lost.
   */
  private async down(view: PageView, def: KeyDef, modifiers = 0): Promise<void> {
    const text = def.text || undefined;
    await cdp(view, 'Input.dispatchKeyEvent', {
      type: 'keyDown',
      ...keyFields(def, modifiers),
      text,
      unmodifiedText: text,
    });
  }

  /** Sends the matching key up. */
  private async up(view: PageView, def: KeyDef, modifiers = 0): Promise<void> {
    await cdp(view, 'Input.dispatchKeyEvent', { type: 'keyUp', ...keyFields(def, modifiers) });
  }

  /** Presses and releases one key, held for a human-length moment. */
  async press(view: PageView, key: string, modifiers = 0): Promise<void> {
    const def = keyDef(key);
    await this.down(view, def, modifiers);
    await sleep(jitter(KEY_HOLD));
    await this.up(view, def, modifiers);
  }

  /** Types one character: down and up, with Shift when it is upper case. */
  private async typeChar(view: PageView, ch: string): Promise<void> {
    const def = keyDef(ch);
    const mods = def.shift ? Modifier.SHIFT : 0;
    await this.down(view, def, mods);
    await this.up(view, def, mods);
  }

  /** Types `text` a character at a time, with Shift where needed and a typist's gaps. */
  async type(view: PageView, text: string): Promise<void> {
    let prev = '';
    for (const ch of text) {
      await this.typeChar(view, ch);
      await sleep(typingDelay(ch, prev));
      prev = ch;
    }
  }

  /** Empties the focused field: select everything with the platform's modifier, then Backspace. */
  async clear(view: PageView): Promise<void> {
    await this.press(view, 'a', this.platform === 'darwin' ? Modifier.META : Modifier.CTRL);
    await sleep(jitter(CLEAR_PAUSE));
    await this.press(view, 'Backspace');
    await sleep(jitter(CLEAR_PAUSE));
  }
}
