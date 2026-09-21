/**
 * Human-like keyboard and mouse input over CDP. Facade over main/input/: key
 * events and typing (keyboard.cjs), Bézier mouse paths (mouse.cjs) and the
 * timing between them (timing.cjs).
 */
const { keyDef, cdpPressKey, cdpTypeText, cdpClearField } = require('./input/keyboard.cjs');
const { cdpMouseMove, cdpClick, cdpScroll } = require('./input/mouse.cjs');
const { sleep, typingDelay } = require('./input/timing.cjs');

module.exports = {
  keyDef,
  typingDelay,
  sleep,
  cdpPressKey,
  cdpTypeText,
  cdpClearField,
  cdpMouseMove,
  cdpClick,
  cdpScroll,
};
