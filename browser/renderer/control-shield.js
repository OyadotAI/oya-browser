/**
 * The control shield's show. While an agent reads the page, light flows around
 * its edge and the Oya orb comes alive; then a soft wash passes down the page
 * and lights up what the agent found.
 * It is drawn here, over the page, so the site never sees any of it. The main process calls `window.oyaShield(update)`.
 */
/* global RendererConstants */

/** Oya's colours for each kind of element: green to click, blue to type in, amber to choose from. */
const TYPE_COLORS = Object.freeze({
  link: '#39ed35',
  button: '#39ed35',
  input: '#6cb4ff',
  textarea: '#6cb4ff',
  editable: '#6cb4ff',
  select: '#f5a623',
});

/** The companion's face and what it says. */
const Companion = {
  /** Sets the mood ('scanning', 'found' or '') and the words in the bubble; no words hides it. */
  say(mood, text, busy = false) {
    const el = document.getElementById('companion');
    el.className = `companion ${mood} ${text ? 'talking' : ''}`.trim();
    const words = document.getElementById('bubble-text');
    words.textContent = text;
    words.classList.toggle('shimmer', busy);
    Companion.arrive(words);
  },

  /** Plays the words' arrival again: the class comes off, a reflow forgets it, and it goes back on. */
  arrive(words) {
    words.classList.remove('enter');
    void words.offsetWidth;
    words.classList.add('enter');
  },
};

/** The scan and outlines of the current analysis. */
const Show = {
  /** When the current scan began (ms since the epoch), or 0 when none is running. */
  scanStart: 0,
  /** Bumped by each update, so a timer left by an older one does nothing. */
  generation: 0,

  /** One update from the main process: `{ phase: 'scan' }` or `{ phase: 'found', boxes }`. */
  update(update) {
    Show.generation += 1;
    if (Object.hasOwn(PHASES, update?.phase)) PHASES[update.phase](update);
  },

  /** Runs `fn` after `ms`, unless a newer update has arrived by then. */
  later(ms, fn) {
    const generation = Show.generation;
    setTimeout(() => generation === Show.generation && fn(), Math.max(0, ms));
  },

  /** The analysis began: wake the grid, sweep the beam and say so. */
  scan() {
    Show.scanStart = Date.now();
    Show.clear();
    document.body.classList.remove('revealing');
    document.body.classList.add('scanning');
    Companion.say('scanning', 'Reading the page', true);
  },

  /** The analysis is back: let the scan finish its minimum, then outline what it found. */
  finish({ boxes }) {
    const shown = Show.scanStart ? Date.now() - Show.scanStart : RendererConstants.SHIELD_MIN_SCAN_MS;
    Show.later(RendererConstants.SHIELD_MIN_SCAN_MS - shown, () => Show.found(boxes || []));
  },

  /** A soft wash passes down the page, lighting up each element as it reaches it, and Oya says how many. */
  found(boxes) {
    Show.scanStart = 0;
    Show.reveal();
    Show.outline(boxes);
    Companion.say('found', Show.summary(boxes.length));
    Show.later(RendererConstants.SHIELD_REVEAL_MS + RendererConstants.SHIELD_HOLD_MS, Show.fade);
  },

  /** Turns the edge light into the reveal: the wash passes once, then the light lets go. */
  reveal() {
    const body = document.body;
    body.style.setProperty('--reveal', `${RendererConstants.SHIELD_REVEAL_MS}ms`);
    body.classList.remove('scanning');
    body.classList.add('revealing');
    Show.later(RendererConstants.SHIELD_REVEAL_MS, () => body.classList.remove('revealing'));
  },

  /** What the companion says about `count` elements. */
  summary(count) {
    if (!count) return 'Nothing to interact with here';
    return `Found ${count} element${count === 1 ? '' : 's'}`;
  },

  /** Draws one outline per box, top to bottom, left to right. */
  outline(boxes) {
    const stage = document.getElementById('stage');
    const ordered = [...boxes].sort((a, b) => a.y - b.y || a.x - b.x);
    stage.replaceChildren(...ordered.map(Show.box));
  },

  /** One outline, coloured by its element's kind, lighting up when the wash reaches it. */
  box(box) {
    const el = document.createElement('div');
    el.className = 'box';
    el.dataset.id = String(box.id);
    Object.assign(el.style, { left: `${box.x}px`, top: `${box.y}px`, width: `${box.w}px`, height: `${box.h}px` });
    el.style.setProperty('--c', TYPE_COLORS[box.type] || TYPE_COLORS.button);
    el.style.setProperty('--d', `${Math.round(Show.beamReaches(box.y))}ms`);
    return el;
  },

  /** Roughly when the wash, crossing the viewport, reaches `y`. */
  beamReaches(y) {
    const share = Math.min(1, Math.max(0, y) / Math.max(1, window.innerHeight || 0));
    return share * RendererConstants.SHIELD_REVEAL_MS;
  },

  /** Fades the outlines out, then puts the companion back to rest. */
  fade() {
    const stage = document.getElementById('stage');
    stage.style.setProperty('--fade', `${RendererConstants.SHIELD_FADE_MS}ms`);
    stage.classList.add('leaving');
    Show.later(RendererConstants.SHIELD_FADE_MS, Show.rest);
  },

  /** No outlines, and a quiet companion. */
  rest() {
    Show.clear();
    document.body.classList.remove('revealing');
    Companion.say('', '');
  },

  /** Removes every outline and makes the stage visible for the next ones. */
  clear() {
    const stage = document.getElementById('stage');
    stage.replaceChildren();
    stage.classList.remove('leaving');
  },
};

/** The main process's updates, by phase. */
const PHASES = { scan: Show.scan, found: Show.finish };

window.oyaShield = Show.update;
