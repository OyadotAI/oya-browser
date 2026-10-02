/**
 * The control shield's show. While an agent reads the page, it dims under smoked
 * glass, an aurora drifts around its edge and the Oya orb comes alive; then a
 * plane of light passes down the page and each element the agent found opens a
 * window of full light in the glass, ringed by a rim of its colour.
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

/**
 * The veil's smoke, for a light page and a dark one: lighter in the middle and darker
 * towards the edges, like a vignette. A light page takes a thin cool smoke, so it dims
 * without going grey; a dark page needs a deep black one, or the dim would not show.
 */
const VEIL_SMOKE = Object.freeze({
  light: Object.freeze({ middle: 'rgba(6, 14, 20, 0.2)', edge: 'rgba(6, 14, 20, 0.46)' }),
  dark: Object.freeze({ middle: 'rgba(0, 0, 0, 0.5)', edge: 'rgba(0, 0, 0, 0.74)' }),
});

/** The light a window lets in on a dark page, where clearing the smoke alone would leave it as dark as the page: a faint mint. */
const VEIL_LIFT = 'rgba(166, 255, 201, 0.08)';

/**
 * The dim over the page, with a soft window over each element the agent found. It is
 * one canvas, drawn only while a window is opening or gliding, so a hundred windows cost
 * one layer and no frame at rest. Where there is no canvas (tests), it does nothing.
 */
const Veil = {
  /** The windows, by element id: `{ box, from, at, movedAt }`, times from animation frames. */
  holes: new Map(),
  /** Whether a frame is already asked for. */
  pending: false,
  /** The page under the veil: 'light' or 'dark', as the main process measured it. */
  tone: 'light',

  /** Opens a window over one element; it grows in from its middle on the next frames. */
  open(box) {
    Veil.holes.set(String(box.id), { box, from: box, at: null, movedAt: null });
    Veil.ask();
  },

  /** The elements moved: each window glides from where it is drawn now to where its element is. */
  move(boxes) {
    const at = new Map(boxes.map((box) => [String(box.id), box]));
    for (const [id, hole] of Veil.holes) Veil.follow(hole, at.get(id));
    Veil.ask();
  },

  /** One window follows its element: it glides to where the element is now, closes when it is gone, and opens again if it comes back. */
  follow(hole, box) {
    if (!box) {
      if (!hole.closing) Object.assign(hole, { closing: true, closedAt: null });
      return;
    }
    if (hole.closing) Object.assign(hole, { closing: false, closedAt: null, at: null });
    Object.assign(hole, { from: hole.drawn || hole.box, box, movedAt: null });
  },

  /** Closes every window. */
  clear() {
    Veil.holes.clear();
    Veil.ask();
  },

  /** Asks for one frame, once. */
  ask() {
    if (Veil.pending) return;
    Veil.pending = true;
    window.requestAnimationFrame(Veil.draw);
  },

  /** Draws the smoke, cuts every window out of it, and asks for another frame while any is still moving. */
  draw(now) {
    Veil.pending = false;
    const ctx = Veil.context();
    if (!ctx) return;
    Veil.smoke(ctx);
    let moving = false;
    for (const hole of Veil.holes.values()) moving = Veil.cut(ctx, hole, now) || moving;
    if (Veil.tone === 'dark') Veil.lift(ctx);
    if (moving) Veil.ask();
  },

  /** Fills the whole canvas with smoke, then sets the context to cut windows out of it. */
  smoke(ctx) {
    const all = Veil.page();
    Object.assign(ctx, { globalCompositeOperation: 'copy', globalAlpha: 1, fillStyle: Veil.vignette(ctx, all) });
    ctx.fillRect(all.x, all.y, all.w, all.h);
    // A cut takes away as much as its own colour covers, so it must be solid to clear the smoke fully.
    Object.assign(ctx, { globalCompositeOperation: 'destination-out', fillStyle: '#000' });
  },

  /** The smoke's fill: clear-ish around the middle of `all`, darkest at its corners. */
  vignette(ctx, all) {
    const { x, y } = Show.middle(all);
    const reach = Math.hypot(all.w, all.h) * RendererConstants.SHIELD_HALF;
    const clear = reach * RendererConstants.SHIELD_VEIL_CLEAR_SHARE;
    const fill = ctx.createRadialGradient(x, y, clear, x, y, reach);
    const smoke = VEIL_SMOKE[Veil.tone] || VEIL_SMOKE.light;
    fill.addColorStop(0, smoke.middle);
    fill.addColorStop(1, smoke.edge);
    return fill;
  },

  /** The canvas's area in page pixels: the window, and the bleed past each edge. */
  page() {
    const view = { x: 0, y: 0, w: window.innerWidth || 0, h: window.innerHeight || 0 };
    return Veil.outset(view, RendererConstants.SHIELD_VEIL_BLEED_PX);
  },

  /** The canvas's context, sized to the window in device pixels, or null where there is none. */
  context() {
    const canvas = document.getElementById('veil');
    const ctx = canvas?.getContext?.('2d');
    if (ctx) Veil.fit(canvas, ctx, window.devicePixelRatio || 1);
    return ctx || null;
  },

  /** Sizes the canvas to the page in device pixels, and draws in page pixels with the bleed's corner at -bleed. */
  fit(canvas, ctx, ratio) {
    const all = Veil.page();
    const size = { width: Math.round(all.w * ratio), height: Math.round(all.h * ratio) };
    if (canvas.width !== size.width || canvas.height !== size.height) Object.assign(canvas, size);
    ctx.setTransform(ratio, 0, 0, ratio, -all.x * ratio, -all.y * ratio);
  },

  /** Cuts one window: it grows open from a little inside its element, and glides after a move. Says whether it is still moving. */
  cut(ctx, hole, now) {
    Veil.stamp(hole, now);
    const glide = Veil.eased((now - hole.movedAt) / RendererConstants.SHIELD_VEIL_GLIDE_MS);
    const open = Veil.strength(hole, now);
    Object.assign(hole, { drawn: Veil.between(hole.from, hole.box, glide), open });
    if (open > 0) Veil.window(ctx, hole.drawn, open);
    return hole.closing ? open > 0 : open < 1 || glide < 1;
  },

  /** Notes, on a window's first frame since it opened, moved or began to close, when that began. */
  stamp(hole, now) {
    hole.at ??= now;
    hole.movedAt ??= now;
    if (hole.closing) hole.closedAt ??= now;
  },

  /** How open a window is now: opening eased in, and closing eased out at the same pace once its element has gone. */
  strength(hole, now) {
    const ms = RendererConstants.SHIELD_VEIL_OPEN_MS;
    const opened = Veil.eased((now - hole.at) / ms);
    return hole.closing ? Math.min(opened, 1 - Veil.eased((now - hole.closedAt) / ms)) : opened;
  },

  /** One window, `open` of the way to full size and strength: a soft falloff of light around it, then the clear window itself. */
  window(ctx, box, open) {
    for (const { px, share } of RendererConstants.SHIELD_VEIL_FALLOFF) Veil.shape(ctx, box, px, open, open * share);
    Veil.shape(ctx, box, 0, open, open);
  },

  /** Fills a rounded box `px` past the window's edge, grown `open` of the way from its starting size, at `alpha`. */
  shape(ctx, box, px, open, alpha) {
    const { SHIELD_VEIL_PAD_PX: pad, SHIELD_VEIL_RADIUS_PX: radius, SHIELD_VEIL_START: start } = RendererConstants;
    const r = Veil.scaled(Veil.outset(box, pad + px), start + (1 - start) * open);
    ctx.globalAlpha = alpha;
    ctx.beginPath();
    ctx.roundRect(r.x, r.y, r.w, r.h, radius + px);
    ctx.fill();
  },

  /** On a dark page, lets a faint light into every window, so what the agent found stands out from the dark around it. */
  lift(ctx) {
    Object.assign(ctx, { globalCompositeOperation: 'source-over', fillStyle: VEIL_LIFT });
    for (const hole of Veil.holes.values()) if (hole.drawn) Veil.shape(ctx, hole.drawn, 0, hole.open, hole.open);
  },

  /** A box grown by `px` on every side. */
  outset(box, px) {
    return { x: box.x - px, y: box.y - px, w: box.w + px + px, h: box.h + px + px };
  },

  /** A box scaled by `k` about its middle. */
  scaled(box, k) {
    const middle = Show.middle(box);
    const half = RendererConstants.SHIELD_HALF;
    return { x: middle.x - box.w * k * half, y: middle.y - box.h * k * half, w: box.w * k, h: box.h * k };
  },

  /** Where a box is `t` of the way from `a` to `b`. */
  between(a, b, t) {
    const at = (k) => a[k] + (b[k] - a[k]) * t;
    return { x: at('x'), y: at('y'), w: at('w'), h: at('h') };
  },

  /** A fraction of the way through, clamped and eased out (fast, then settling). */
  eased(t) {
    const clamped = Math.min(1, Math.max(0, t || 0));
    return 1 - (1 - clamped) ** RendererConstants.SHIELD_VEIL_EASE_POWER;
  },
};

/** The companion's face and what it says. */
const Companion = {
  /** The mood shown now: 'scanning', 'found', 'acting' or ''. */
  mood: '',

  /** Sets the mood and the words in the caption; no words hides it (the last words fade out with it). `busy` quickens the voice line. */
  say(mood, text, busy = false) {
    Companion.mood = mood;
    const classes = ['companion', mood, text && 'talking', busy && 'busy'];
    document.getElementById('companion').className = classes.filter(Boolean).join(' ');
    if (text) Companion.words(text);
  },

  /** Writes the caption as one span per word, so each rises into place a moment after the one before. */
  words(text) {
    const parts = String(text).split(' ');
    const spans = parts.map((word, i) => Companion.word(i < parts.length - 1 ? `${word} ` : word, i));
    document.getElementById('bubble-text').replaceChildren(...spans);
  },

  /** One word of the caption, with its place in line for its delay. */
  word(text, i) {
    const span = Object.assign(document.createElement('span'), { className: 'w', textContent: text });
    span.style.setProperty('--i', String(i));
    return span;
  },

  /** Changes the words in place, without playing their arrival again: for a count ticking up. */
  count(text) {
    document.getElementById('bubble-text').textContent = text;
  },

  /** Sends a ring of light out from the orb: the class comes off, a reflow forgets it, and it goes back on. */
  pulse() {
    const ring = document.querySelector('.orb .pulse');
    if (!ring) return;
    ring.classList.remove('go');
    void ring.offsetWidth;
    ring.classList.add('go');
  },

  /** Goes quiet, unless an action has taken the caption since: the show's end must not cut off what Oya is doing. */
  rest() {
    if (Companion.mood !== 'acting') Companion.say('', '');
  },
};

/**
 * What Oya is doing between reads: each action the agent takes is said in the caption,
 * the orb pulses, and light reaches out to the element it acts on and locks a ring on it.
 */
const Act = {
  /** The timer that quiets the caption once the agent has been still a while. */
  timer: 0,

  /** One action from the main process: `{ phase: 'act', text, box }`, the box when the element could be measured. */
  act({ text, box, changes }) {
    if (changes) Show.dismiss();
    Companion.say('acting', text);
    Companion.pulse();
    if (box) Act.target(box);
    clearTimeout(Act.timer);
    Act.timer = setTimeout(Act.quiet, RendererConstants.SHIELD_ACT_HOLD_MS);
  },

  /** The run ended: whatever the show still drew lets go, and Oya says it is done before going quiet. */
  end() {
    Show.dismiss();
    Companion.say('acting', 'Done');
    clearTimeout(Act.timer);
    Act.timer = setTimeout(Act.quiet, RendererConstants.SHIELD_DONE_MS);
  },

  /** The agent has been still: the caption goes, unless a read has taken it over. */
  quiet() {
    if (Companion.mood === 'acting') Companion.say('', '');
  },

  /** Light flies from the orb to the element, then a ring locks on to it; both are removed once played. */
  target(box) {
    const layer = document.getElementById('targets');
    const ring = Object.assign(document.createElement('div'), { className: 'target' });
    Show.place(ring, box);
    ring.style.setProperty('--c', TYPE_COLORS[box.type] || TYPE_COLORS.button);
    const orb = Show.orbCentre();
    const shown = orb ? [ring, Show.flight(orb, Show.middle(box), box.type, 0)] : [ring];
    layer.append(...shown);
    setTimeout(() => shown.forEach((el) => el.remove()), RendererConstants.SHIELD_TARGET_MS);
  },
};

/** The scan and outlines of the current analysis. */
const Show = {
  /** When the current scan began (ms since the epoch), or 0 when none is running. */
  scanStart: 0,
  /** When the reveal's pass reaches the bottom of the page (ms since the epoch), or 0 when none is running. */
  passEnds: 0,
  /** Bumped by each new scan or result, so a timer left by an older one does nothing. A move keeps the show going. */
  generation: 0,
  /** Where the found elements sit now: the result, then each move the main process measures after it. */
  latest: [],
  /** Where this show's number badges sit, so a badge that would land on another stays hidden. */
  tags: [],

  /** One update from the main process: `{ phase: 'scan' }`, `{ phase: 'found', boxes }` or `{ phase: 'move', boxes }`. */
  update(update) {
    // Only a new read restarts the show; a move or an action plays over it.
    if (update?.phase === 'scan' || update?.phase === 'found') Show.generation += 1;
    if (Object.hasOwn(PHASES, update?.phase)) PHASES[update.phase](update);
  },

  /** Runs `fn` after `ms`, unless a newer update has arrived by then. */
  later(ms, fn) {
    const generation = Show.generation;
    setTimeout(() => generation === Show.generation && fn(), Math.max(0, ms));
  },

  /** The analysis began: wake the grid, sweep the beam and say so. */
  scan() {
    const wait = Show.passEnds - Date.now();
    if (wait <= 0) return Show.read();
    // A pass is still crossing the page: it finishes first, so the beam never jumps back to the top halfway down.
    Show.scanStart = Show.passEnds;
    Show.later(wait, Show.read);
  },

  /** The reading itself: the beam sweeps the page on a steady loop while the companion says so. */
  read() {
    Show.scanStart = Date.now();
    Show.passEnds = 0;
    Show.letGo();
    document.body.style.setProperty('--loop', `${RendererConstants.SHIELD_SCAN_LOOP_MS}ms`);
    document.body.classList.remove('revealing', 'lit');
    document.body.classList.add('scanning');
    Companion.say('scanning', 'Reading the page', true);
  },

  /** The analysis is back: once the scan has run its minimum, and its beam has reached the bottom, outline what it found. */
  finish({ boxes, tone }) {
    Show.latest = boxes || [];
    Veil.tone = tone === 'dark' ? 'dark' : 'light';
    Show.later(Show.untilReveal(), () => Show.found(Show.latest));
  },

  /**
   * How long until the reveal may start: the scan's minimum, then the end of the beam's
   * current sweep, so the reveal's pass follows it as the next sweep from the top rather
   * than the beam jumping back from wherever it was. No scan running, no wait.
   */
  untilReveal() {
    if (!Show.scanStart) return 0;
    const { SHIELD_MIN_SCAN_MS: min, SHIELD_SCAN_LOOP_MS: loop } = RendererConstants;
    const elapsed = Date.now() - Show.scanStart;
    const atLeast = Math.max(min, elapsed);
    return Math.ceil(atLeast / loop) * loop - elapsed;
  },

  /** The page moved under the outlines: each glides to where its element is now, and one whose element is gone fades. */
  move({ boxes }) {
    Show.latest = boxes || [];
    Veil.move(Show.latest);
    const at = new Map(Show.latest.map((box) => [String(box.id), box]));
    for (const el of document.getElementById('stage').children) {
      const box = at.get(el.dataset.id);
      el.classList.toggle('gone', !box);
      if (box) Show.place(el, box);
    }
  },

  /** The beam passes down the page, locking on to each element as it reaches it, and Oya counts them. */
  found(boxes) {
    Show.clear();
    Show.reveal();
    const shown = Show.plan(boxes);
    const every = Math.max(1, Math.ceil(shown.length / RendererConstants.SHIELD_SPARKS_MAX));
    const counted = (i) => Show.counted(Math.round(((i + 1) * boxes.length) / shown.length));
    Companion.say('found', shown.length ? Show.counted(0) : 'Nothing to interact with here');
    shown.forEach((box, i) => Show.later(Show.beamReaches(box.y), () => Show.lock(box, counted(i), i % every === 0)));
    Show.later(RendererConstants.SHIELD_REVEAL_MS + RendererConstants.SHIELD_HOLD_MS, Show.fade);
  },

  /** Turns the edge light into the reveal: the wash passes once, then the light lets go. */
  reveal() {
    const body = document.body;
    const { SHIELD_REVEAL_MS: reveal, SHIELD_FADE_MS: fade } = RendererConstants;
    Object.assign(Show, { scanStart: 0, passEnds: Date.now() + reveal });
    // Set now, while the stage is empty: setting them as the fade begins would restyle every outline at once.
    Object.entries({ reveal, fade }).forEach(([name, ms]) => body.style.setProperty(`--${name}`, `${ms}ms`));
    body.classList.remove('scanning');
    body.classList.add('revealing', 'lit');
    Show.later(reveal, () => body.classList.remove('revealing'));
  },

  /** What the companion says once it has counted `count` elements. */
  counted(count) {
    return `Found ${count} element${count === 1 ? '' : 's'}`;
  },

  /**
   * The beam reached one element: its outline locks on where the element is now, Oya's
   * count goes up, and, for one in every few, a spark flies to the orb.
   */
  lock(box, words, sparks) {
    const now = Show.latest.find((b) => b.id === box.id);
    // Its element has gone since it was measured (the page moved on): it is counted, but nothing is drawn where it was.
    if (!now) return Companion.count(words);
    document.getElementById('stage').append(Show.box(now));
    Veil.open(now);
    Companion.count(words);
    const orb = sparks && Show.orbCentre();
    if (orb) document.getElementById('sparks').append(Show.spark(now, orb));
  },

  /** One outline with its number (tucked in at the window's edge, left out where it would land on another), coloured by its element's kind. */
  box(box) {
    const el = document.createElement('div');
    const tag = [Show.tucked(box) && 'tucked', !Show.roomForTag(box) && 'quiet'];
    el.className = ['box', ...tag.filter(Boolean)].join(' ');
    el.dataset.id = String(box.id);
    el.append(Show.badge(box.id));
    Show.place(el, box);
    el.style.setProperty('--c', TYPE_COLORS[box.type] || TYPE_COLORS.button);
    return el;
  },

  /** The outlines this show draws, top to bottom; a page with many of them is marked busy. */
  plan(boxes) {
    const shown = Show.declutter(boxes).sort((a, b) => a.y - b.y || a.x - b.x);
    document.getElementById('stage').classList.toggle('busy', shown.length > RendererConstants.SHIELD_BUSY_COUNT);
    return shown;
  },

  /**
   * Keeps what a person can read on a busy page. Page-sized boxes go, and so does any
   * box that wraps, or mostly repeats, a smaller one already kept: a card around its
   * links, a list around its items. Smallest first, so the innermost controls stay.
   */
  declutter(boxes) {
    const page = (window.innerWidth || 0) * (window.innerHeight || 0);
    const kept = [];
    for (const box of [...boxes].sort((a, b) => Show.area(a) - Show.area(b))) {
      if (page && Show.area(box) > page * RendererConstants.SHIELD_MAX_BOX_SHARE) continue;
      const wraps = (k) => Show.overlap(box, k) >= Show.area(k) * RendererConstants.SHIELD_WRAP_SHARE;
      if (!kept.some(wraps)) kept.push(box);
    }
    return kept;
  },

  /** A box's area. */
  area(box) {
    return box.w * box.h;
  },

  /** The area two boxes share. */
  overlap(a, b) {
    const w = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
    const h = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
    return Math.max(0, w) * Math.max(0, h);
  },

  /** Whether this box's number badge fits without landing on one already shown; if so, it is claimed. */
  roomForTag(box) {
    const { SHIELD_TAG_PX: size, SHIELD_TAG_OFFSET_PX: offset, SHIELD_TAG_DIGIT_PX: digit } = RendererConstants;
    const shift = Show.tucked(box) ? -RendererConstants.SHIELD_TAG_TUCK_PX : offset;
    const tag = { x: box.x - shift, y: box.y - shift, w: size + digit * (String(box.id).length - 1), h: size };
    if (Show.tags.some((t) => Show.overlap(tag, t) > 0)) return false;
    Show.tags.push(tag);
    return true;
  },

  /** Whether a box hugs the window's top or left edge, so its number sits just inside its corner rather than off the page. */
  tucked(box) {
    const offset = RendererConstants.SHIELD_TAG_OFFSET_PX;
    return box.x < offset || box.y < offset;
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
    return Show.flight(Show.middle(box), orb, box.type, RendererConstants.SHIELD_SPARK_LAG_MS);
  },

  /** A point of light flying on an arc from `from` to `to`, in the colour of a `type` element, leaving after `lag` ms. */
  flight(from, to, type, lag) {
    const el = Object.assign(document.createElement('div'), { className: 'spark' });
    el.append(document.createElement('i'));
    const vars = { '--fx': from.x, '--fy': from.y, '--dx': to.x - from.x, '--dy': to.y - from.y };
    for (const [name, value] of Object.entries(vars)) el.style.setProperty(name, `${Math.round(value)}px`);
    el.style.setProperty('--c', TYPE_COLORS[type] || TYPE_COLORS.button);
    el.style.setProperty('--d', `${lag}ms`);
    return el;
  },

  /** Puts an outline exactly over its element, snapped to device pixels so its hairline stays crisp. */
  place(el, box) {
    const ratio = window.devicePixelRatio || 1;
    const px = (v) => `${Math.round(v * ratio) / ratio}px`;
    el.style.transform = `translate3d(${px(box.x)}, ${px(box.y)}, 0)`;
    Object.assign(el.style, { width: px(box.w), height: px(box.h) });
    Show.reach(el, '--sx', '--sy', RendererConstants.SHIELD_LOCK_REACH_PX, box);
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

  /**
   * The agent changed the page: whatever the show was outlining is out of date, so it
   * lets go now, quickly, and a reveal still waiting for its sweep never plays.
   */
  dismiss() {
    const body = document.body;
    Show.generation += 1;
    Object.assign(Show, { scanStart: 0, passEnds: 0 });
    document.getElementById('stage').classList.add('leaving');
    body.classList.remove('scanning', 'lit');
    body.classList.add('dismissing');
    Show.later(RendererConstants.SHIELD_DISMISS_MS, Show.dismissed);
  },

  /** Fades out whatever the last show left up, rather than cutting it: its outlines fade and its windows in the veil close. */
  letGo() {
    const stage = document.getElementById('stage');
    if (!stage.children.length) return Show.clear();
    stage.classList.add('leaving');
    Veil.move([]);
    document.body.classList.add('dismissing');
    Show.later(RendererConstants.SHIELD_DISMISS_MS, Show.dismissed);
  },

  /** The dismissed show has faded: nothing is left of it. */
  dismissed() {
    Show.clear();
    document.body.classList.remove('revealing', 'dismissing');
  },

  /** Fades the outlines and the veil out, then puts the companion back to rest. */
  fade() {
    document.getElementById('stage').classList.add('leaving');
    document.body.classList.remove('lit');
    Companion.rest();
    Show.later(RendererConstants.SHIELD_FADE_MS, Show.rest);
  },

  /** No outlines, and a quiet companion. */
  rest() {
    Show.clear();
    document.body.classList.remove('revealing');
    Companion.rest();
  },

  /** Removes every outline and makes the stage visible for the next ones. */
  clear() {
    const stage = document.getElementById('stage');
    stage.replaceChildren();
    stage.classList.remove('leaving');
    Show.tags = [];
    Veil.clear();
    document.getElementById('sparks').replaceChildren();
  },
};

/** The main process's updates, by phase. */
const PHASES = { scan: Show.scan, found: Show.finish, move: Show.move, act: Act.act, end: Act.end };

window.oyaShield = Show.update;
