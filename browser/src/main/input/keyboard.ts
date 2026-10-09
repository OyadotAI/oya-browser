/** Native keyboard input: key definitions, presses, and typing with a human cadence. */
import { nativeKeyDown, nativeKeyUp, type NativeKeyboardView as PageView } from './native-keyboard.ts';
import { KEY_DEFS, Modifier, KEY_HOLD, CLEAR_PAUSE, type KeyDef } from './constants.ts';
import { sleep, jitter, typingDelay } from './timing.ts';

/** The logical key definition for a named key or a single character. */
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

/** Types and presses keys on a view, as a person at a keyboard would. */
export class Keyboard {
  /** Keep the existing construction contract; native editing no longer depends on OS accelerator mappings. */
  constructor(_platform: NodeJS.Platform) {}

  /** Send a physical key and its character through the native input surface. */
  private down(view: PageView, def: KeyDef, modifiers = 0): Promise<void> {
    return nativeKeyDown(view, def, modifiers);
  }
  /** Release a physical key; composed Unicode text has no invented key-up. */
  private up(view: PageView, def: KeyDef, modifiers = 0): void {
    nativeKeyUp(view, def, modifiers);
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

  /** Empties the focused field: use native select-all, then Backspace. */
  async clear(view: PageView): Promise<void> {
    if (view.webContents.isDestroyed()) throw new Error('View is destroyed');
    view.webContents.selectAll();
    await sleep(jitter(CLEAR_PAUSE));
    await this.press(view, 'Backspace');
    await sleep(jitter(CLEAR_PAUSE));
  }
}
