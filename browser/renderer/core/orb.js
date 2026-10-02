/**
 * The Oya orb (styled by core/orb.css): built once per place it appears, then told
 * its state as the agent works. Any element written as `<span data-orb="xl">` becomes
 * an orb of that size when the shell loads, so markup can place one without code.
 */
/* exported OyaOrb */

/** The mark's geometry inside the lens: two rings, the back one offset down and right. */
const ORB_MARK = Object.freeze({
  viewBox: '0 0 100 100',
  r: 19,
  width: 9.5,
  rings: Object.freeze([Object.freeze({ name: 'mark-back', c: 53 }), Object.freeze({ name: 'mark-front', c: 47 })]),
});

/** The orb's states, as core/orb.css draws them. */
const ORB_STATES = Object.freeze(['idle', 'thinking', 'acting', 'done', 'failed']);

/** Building and driving orbs. */
const OyaOrb = {
  /** A new orb of `size` ('xs', 'sm', '' for medium, 'lg', 'xl'), resting. */
  create(size = '') {
    const orb = OyaOrb.part('span', 'oya-orb');
    if (size) orb.dataset.size = size;
    orb.dataset.state = 'idle';
    orb.setAttribute('aria-hidden', 'true');
    orb.append(OyaOrb.spin(), OyaOrb.orbit(), OyaOrb.lens(), OyaOrb.part('span', 'oya-orb-pulse'));
    return orb;
  },

  /** Sets what the orb shows; an unknown state rests it. */
  state(orb, state) {
    if (orb) orb.dataset.state = ORB_STATES.includes(state) ? state : 'idle';
  },

  /** Sends one ring of light out from the orb: the class comes off, a reflow forgets it, and it goes back on. */
  pulse(orb) {
    const ring = orb?.querySelector('.oya-orb-pulse');
    if (!ring) return;
    ring.classList.remove('go');
    void ring.offsetWidth;
    ring.classList.add('go');
  },

  /** Turns every `[data-orb]` placeholder under `root` into an orb of its size, keeping its id, classes and starting state (`data-orb-state`). */
  mountAll(root = document) {
    for (const spot of root.querySelectorAll('[data-orb]')) {
      const orb = OyaOrb.create(spot.dataset.orb);
      if (spot.id) orb.id = spot.id;
      if (spot.className) orb.className += ` ${spot.className}`;
      OyaOrb.state(orb, spot.dataset.orbState);
      spot.replaceWith(orb);
    }
  },

  /** Marks the root while the window is hidden, so core/orb.css pauses every loop. */
  followVisibility() {
    const mark = () => document.documentElement.setAttribute('data-hidden', String(!!document.hidden));
    document.addEventListener('visibilitychange', mark);
    mark();
  },

  /** The turning light: the halo and the rim, inside a wrapper that turns faster while Oya thinks. */
  spin() {
    const spin = OyaOrb.part('span', 'oya-orb-spin');
    spin.append(OyaOrb.part('span', 'oya-orb-halo'), OyaOrb.part('span', 'oya-orb-rim'));
    return spin;
  },

  /** The tilted orbit and its one bright point. */
  orbit() {
    const orbit = OyaOrb.part('span', 'oya-orb-orbit');
    orbit.append(document.createElement('i'));
    return orbit;
  },

  /** The glass lens: the circling lights, then the Oya mark over them. */
  lens() {
    const lens = OyaOrb.part('span', 'oya-orb-lens');
    const core = OyaOrb.part('span', 'oya-orb-core');
    core.append(document.createElement('i'), document.createElement('i'));
    lens.append(core, OyaOrb.mark());
    return lens;
  },

  /** The Oya mark inside the lens: the green ring behind, offset, and the white ring in front (it turns while Oya thinks, so it is drawn here, not borrowed). */
  mark() {
    const svg = OyaOrb.svg('svg');
    svg.setAttribute('class', 'oya-orb-mark');
    svg.setAttribute('viewBox', ORB_MARK.viewBox);
    svg.append(...ORB_MARK.rings.map(OyaOrb.ring));
    return svg;
  },

  /** One ring of the mark. */
  ring({ name, c }) {
    const circle = OyaOrb.svg('circle');
    const { r, width } = ORB_MARK;
    for (const [k, v] of Object.entries({ class: name, cx: c, cy: c, r, fill: 'none', 'stroke-width': width }))
      circle.setAttribute(k, String(v));
    return circle;
  },

  /** A new SVG element. */
  svg(tag) {
    return document.createElementNS('http://www.w3.org/2000/svg', tag);
  },

  /** One element of the orb, by tag and class. */
  part(tag, className) {
    const el = document.createElement(tag);
    el.className = className;
    return el;
  },
};
