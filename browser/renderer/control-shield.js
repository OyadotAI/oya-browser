/**
 * The control shield's show. While an agent reads the page, light flows around
 * its edge and the Oya orb comes alive; then a laser beam passes down the page
 * and locks on to what the agent found.
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

  /** Changes the words in place, without playing their arrival again: for a count ticking up. */
  count(text) {
    document.getElementById('bubble-text').textContent = text;
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
  /** Bumped by each new scan or result, so a timer left by an older one does nothing. A move keeps the show going. */
  generation: 0,
  /** Where the found elements sit now: the result, then each move the main process measures after it. */
  latest: [],

  /** One update from the main process: `{ phase: 'scan' }`, `{ phase: 'found', boxes }` or `{ phase: 'move', boxes }`. */
  update(update) {
    if (update?.phase !== 'move') Show.generation += 1;
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
    Show.latest = boxes || [];
    const shown = Show.scanStart ? Date.now() - Show.scanStart : RendererConstants.SHIELD_MIN_SCAN_MS;
    Show.later(RendererConstants.SHIELD_MIN_SCAN_MS - shown, () => Show.found(Show.latest));
  },

  /** The page moved under the outlines: each glides to where its element is now, and one whose element is gone fades. */
  move({ boxes }) {
    Show.latest = boxes || [];
    const at = new Map(Show.latest.map((box) => [String(box.id), box]));
    for (const el of document.getElementById('stage').children) {
      const box = at.get(el.dataset.id);
      el.classList.toggle('gone', !box);
      if (box) Show.place(el, box);
    }
  },

  /** The beam passes down the page, locking on to each element as it reaches it, and Oya counts them. */
  found(boxes) {
    Show.scanStart = 0;
    Show.clear();
    Show.reveal();
    const ordered = [...boxes].sort((a, b) => a.y - b.y || a.x - b.x);
    const every = Math.max(1, Math.ceil(ordered.length / RendererConstants.SHIELD_SPARKS_MAX));
    Companion.say('found', ordered.length ? Show.counted(0) : 'Nothing to interact with here');
    ordered.forEach((box, i) => Show.later(Show.beamReaches(box.y), () => Show.lock(box, i + 1, i % every === 0)));
    Show.later(RendererConstants.SHIELD_REVEAL_MS + RendererConstants.SHIELD_HOLD_MS, Show.fade);
  },

  /** Turns the edge light into the reveal: the wash passes once, then the light lets go. */
  reveal() {
    const body = document.body;
    body.style.setProperty('--reveal', `${RendererConstants.SHIELD_REVEAL_MS}ms`);
    // Set now, while the stage is empty: setting it as the fade begins would restyle every outline at once.
    body.style.setProperty('--fade', `${RendererConstants.SHIELD_FADE_MS}ms`);
    body.classList.remove('scanning');
    body.classList.add('revealing');
    Show.later(RendererConstants.SHIELD_REVEAL_MS, () => body.classList.remove('revealing'));
  },

  /** What the companion says once it has counted `count` elements. */
  counted(count) {
    return `Found ${count} element${count === 1 ? '' : 's'}`;
  },

  /**
   * The beam reached one element: its outline locks on where the element is now, Oya's
   * count goes up, and, for one in every few, a spark flies to the orb.
   */
  lock(box, count, sparks) {
    const now = Show.latest.find((b) => b.id === box.id) || box;
    document.getElementById('stage').append(Show.box(now));
    Companion.count(Show.counted(count));
    const orb = sparks && Show.orbCentre();
    if (orb) document.getElementById('sparks').append(Show.spark(now, orb));
  },

  /** One outline with its number, coloured by its element's kind, lighting up when the wash reaches it. */
  box(box) {
    const el = document.createElement('div');
    el.className = 'box';
    el.dataset.id = String(box.id);
    el.append(...['i', 's', 'u'].map((tag) => document.createElement(tag)), Show.badge(box.id));
    Show.place(el, box);
    el.style.setProperty('--c', TYPE_COLORS[box.type] || TYPE_COLORS.button);
    return el;
  },

  /** An outline's number, pinned to its corner. */
  badge(id) {
    const badge = document.createElement('b');
    badge.textContent = String(id);
    return badge;
  },

  /** The middle of the orb, or null before it is laid out. */
  orbCentre() {
    const r = document.querySelector('.orb')?.getBoundingClientRect();
    const centre = r && Show.middle({ x: r.left, y: r.top, w: r.width, h: r.height });
    return centre && Number.isFinite(centre.x) && Number.isFinite(centre.y) ? centre : null;
  },

  /** The middle of a box. */
  middle(box) {
    const half = RendererConstants.SHIELD_HALF;
    return { x: box.x + box.w * half, y: box.y + box.h * half };
  },

  /** One spark, leaving the middle of its element just after its brackets lock on. */
  spark(box, orb) {
    const el = Object.assign(document.createElement('div'), { className: 'spark' });
    el.append(document.createElement('i'));
    const from = Show.middle(box);
    const vars = { '--fx': from.x, '--fy': from.y, '--dx': orb.x - from.x, '--dy': orb.y - from.y };
    for (const [name, value] of Object.entries(vars)) el.style.setProperty(name, `${Math.round(value)}px`);
    el.style.setProperty('--c', TYPE_COLORS[box.type] || TYPE_COLORS.button);
    el.style.setProperty('--d', `${RendererConstants.SHIELD_SPARK_LAG_MS}ms`);
    return el;
  },

  /** Puts an outline exactly over its element, snapped to device pixels so its hairline stays crisp. */
  place(el, box) {
    const ratio = window.devicePixelRatio || 1;
    const px = (v) => `${Math.round(v * ratio) / ratio}px`;
    el.style.transform = `translate3d(${px(box.x)}, ${px(box.y)}, 0)`;
    Object.assign(el.style, { width: px(box.w), height: px(box.h) });
    Show.reach(el, '--sx', '--sy', RendererConstants.SHIELD_LOCK_REACH_PX, box);
    Show.reach(el, '--rx', '--ry', RendererConstants.SHIELD_RIPPLE_REACH_PX, box);
  },

  /** Sets the scale, across and down, that makes an outline `px` wider and taller than its element. */
  reach(el, x, y, px, box) {
    el.style.setProperty(x, String(1 + px / Math.max(1, box.w)));
    el.style.setProperty(y, String(1 + px / Math.max(1, box.h)));
  },

  /** When the beam, crossing the viewport at an even pace, reaches `y`. */
  beamReaches(y) {
    const share = Math.min(1, Math.max(0, y) / Math.max(1, window.innerHeight || 0));
    return share * RendererConstants.SHIELD_REVEAL_MS;
  },

  /** Fades the outlines out, then puts the companion back to rest. */
  fade() {
    document.getElementById('stage').classList.add('leaving');
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
    document.getElementById('sparks').replaceChildren();
  },
};

/** The main process's updates, by phase. */
const PHASES = { scan: Show.scan, found: Show.finish, move: Show.move };

window.oyaShield = Show.update;
