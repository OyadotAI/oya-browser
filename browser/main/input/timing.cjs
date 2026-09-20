/** Human-like timing: randomised pauses and the gap between keystrokes. */
const { TYPE_BASE, WORD_PAUSE, SPECIAL_PAUSE, HESITATION, HESITATION_CHANCE } = require('./constants.cjs');

/** Resolves after `ms` milliseconds. */
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** A random duration in a `{ base, spread }` range. */
const jitter = ({ base, spread }) => base + Math.random() * spread;

/** How long to wait after typing `ch`, given the character before it. */
function typingDelay(ch, prev) {
  let ms = jitter(TYPE_BASE);
  if (prev === ' ') ms += jitter(WORD_PAUSE);
  if (!/[a-zA-Z0-9 ]/.test(ch)) ms += jitter(SPECIAL_PAUSE);
  if (Math.random() < HESITATION_CHANCE) ms += jitter(HESITATION);
  return ms;
}

module.exports = { sleep, jitter, typingDelay };
