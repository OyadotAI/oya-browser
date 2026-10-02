/**
 * The launch: once per app start, Oya wakes up. Its two rings drift together into
 * the mark, light runs around the window's edge, the orb opens its eye, and the
 * stage dissolves into whatever the shell shows. It never holds the person up: a
 * click or a key ends it at once, and with reduced motion it does not play.
 */
/* global Dom, RendererConstants */
/* exported Launch */

/** The launch moment. */
const Launch = {
  /** The timer that ends the launch, while it plays. */
  timer: 0,

  /** The launch is already playing from the first paint (its markup starts so); this times its end, or ends it at once with reduced motion. */
  play() {
    if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) return (Dom.byId('launch').hidden = true);
    Launch.timer = setTimeout(Launch.end, RendererConstants.LAUNCH_MS);
    for (const type of ['pointerdown', 'keydown']) window.addEventListener(type, Launch.end, { once: true });
  },

  /** Dissolves the stage, its orb gliding onto the orb of whatever shows under it, then takes it away. */
  end() {
    const stage = Dom.byId('launch');
    if (stage.hidden || stage.classList.contains('leaving')) return;
    clearTimeout(Launch.timer);
    stage.classList.remove('playing');
    stage.classList.add('leaving');
    // The orb's entrance is off once the stage is leaving; a layout pass first lets its move be a transition.
    void stage.offsetWidth;
    Launch.handOff(Dom.byId('launch-orb'), Launch.landing());
    setTimeout(() => (stage.hidden = true), RendererConstants.LAUNCH_LEAVE_MS);
  },

  /** The orb the launch hands over to: the start page's, or the welcome screen's; none when neither shows. */
  landing() {
    const start = !Dom.byId('start-page').hidden && Dom.byId('start-orb');
    return start || document.querySelector('body.mode-setup .welcome-stage .oya-orb');
  },

  /** Moves and scales the launch orb onto `target`, so the two are one orb as the stage dissolves. */
  handOff(orb, target) {
    const [from, to] = [orb?.getBoundingClientRect(), target?.getBoundingClientRect?.()];
    if (!from?.width || !to?.width) return;
    orb.style.transform = `translate(${to.left - from.left}px, ${to.top - from.top}px) scale(${to.width / from.width})`;
  },
};
