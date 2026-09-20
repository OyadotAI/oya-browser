/** Keyboard input over CDP: key definitions, presses, and typing with a human cadence. */
const { cdp } = require('../cdp.cjs');
const { KEY_DEFS, Modifier, KEY_HOLD, CLEAR_PAUSE } = require('./constants.cjs');
const { sleep, jitter, typingDelay } = require('./timing.cjs');

/** The CDP key event fields for a named key or a single character. */
function keyDef(ch) {
  if (KEY_DEFS[ch]) return { ...KEY_DEFS[ch] };
  const upper = ch.toUpperCase();
  const isLetter = /^[a-zA-Z]$/.test(ch);
  const isDigit = /^[0-9]$/.test(ch);
  const code = isLetter ? 'Key' + upper : isDigit ? 'Digit' + ch : '';
  const keyCode = isLetter ? upper.charCodeAt(0) : isDigit ? ch.charCodeAt(0) : 0;
  return { key: ch, code, keyCode, text: ch, shift: ch !== ch.toLowerCase() && ch === upper };
}

/** The fields a key's down and up events share. */
const keyFields = (def, modifiers) => ({
  modifiers,
  windowsVirtualKeyCode: def.keyCode,
  nativeVirtualKeyCode: def.keyCode,
  key: def.key,
  code: def.code,
});

/** Sends a key down; a key that produces text is a `keyDown`, others a `rawKeyDown`. */
async function cdpKeyDown(view, def, modifiers = 0) {
  const isChar = !!def.text;
  await cdp(view, 'Input.dispatchKeyEvent', {
    type: isChar ? 'keyDown' : 'rawKeyDown',
    ...keyFields(def, modifiers),
    text: isChar ? def.text : undefined,
    unmodifiedText: isChar ? def.text : undefined,
  });
}

/** Sends the matching key up. */
async function cdpKeyUp(view, def, modifiers = 0) {
  await cdp(view, 'Input.dispatchKeyEvent', { type: 'keyUp', ...keyFields(def, modifiers) });
}

/** Presses and releases one key, held for a human-length moment. */
async function cdpPressKey(view, key, modifiers = 0) {
  const def = keyDef(key);
  await cdpKeyDown(view, def, modifiers);
  await sleep(jitter(KEY_HOLD));
  await cdpKeyUp(view, def, modifiers);
}

/** Types one character: down and up, with Shift when it is upper case. */
async function typeChar(view, ch) {
  const def = keyDef(ch);
  const mods = def.shift ? Modifier.SHIFT : 0;
  await cdpKeyDown(view, def, mods);
  await cdpKeyUp(view, def, mods);
}

/** Types `text` a character at a time, with Shift where needed and a typist's gaps. */
async function cdpTypeText(view, text) {
  let prev = '';
  for (const ch of text) {
    await typeChar(view, ch);
    await sleep(typingDelay(ch, prev));
    prev = ch;
  }
}

/** Select-all with the platform's modifier: Cmd on macOS, Ctrl elsewhere. */
async function cdpSelectAll(view) {
  const mod = process.platform === 'darwin' ? Modifier.META : Modifier.CTRL;
  await cdpPressKey(view, 'a', mod);
}

/** Empties the focused field: select everything, then Backspace. */
async function cdpClearField(view) {
  await cdpSelectAll(view);
  await sleep(jitter(CLEAR_PAUSE));
  await cdpPressKey(view, 'Backspace');
  await sleep(jitter(CLEAR_PAUSE));
}

module.exports = { keyDef, cdpPressKey, cdpTypeText, cdpClearField };
