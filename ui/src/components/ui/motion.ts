/** Progressive animation owns its observer, interaction listeners and controls while server content stays visible. */
import { animate } from 'framer-motion';
import {
  MOTION_EASE,
  MOTION_INTERACTIONS,
  MOTION_MAX_ORDER,
  MOTION_MOBILE_DISTANCE,
  MOTION_MOBILE_QUERY,
  MOTION_MOBILE_SCALE,
  MOTION_PRESETS,
  MOTION_REDUCED_QUERY,
  MOTION_STAGGER,
  MOTION_THRESHOLD,
  type RevealPreset,
} from './constants';

/** A region owns every resource required to animate or settle it. */
interface Session {
  /** Only this inner layer receives transform and opacity changes. */ element: HTMLElement;
  /** Current system preference is checked again before every playback. */ preference: MediaQueryList;
  /** A viewport entrance never repeats. */ played: boolean;
  /** Queued browser callbacks become inert after cleanup. */ disposed: boolean;
  /** Every listener is released by the same owner. */ cleanup: (() => void)[];
  /** Framer owns the finite browser animation. */ control: ReturnType<typeof animate> | null;
  /** Only reveal sessions need viewport observation. */ observer: IntersectionObserver | null;
}

/** Without browser capabilities, readable server content needs no enhancement. */
const noop = () => {};

/** Initial state is deliberately visible and allocates no browser resources. */
const sessionFor = (element: HTMLElement, preference: MediaQueryList): Session => ({
  element,
  preference,
  played: false,
  disposed: false,
  cleanup: [],
  control: null,
  observer: null,
});

/** Unsupported or throwing media APIs leave content in its final state. */
function createSession(element: HTMLElement): Session | null {
  try {
    if (typeof window.matchMedia !== 'function') return null;
    return sessionFor(element, window.matchMedia(MOTION_REDUCED_QUERY));
  } catch {
    return null;
  }
}

/** A cosmetic engine failure must not prevent cleanup or leave hidden content. */
function safely(operation: () => void) {
  try {
    operation();
  } catch {
    /* Base styles are the accessible fallback. */
  }
}

/** Completing then stopping removes temporary styles without retaining a moving focus target. */
function settle(session: Session) {
  safely(() => session.control?.complete());
  safely(() => session.control?.stop());
  session.control = null;
  session.element.style.removeProperty('opacity');
  session.element.style.removeProperty('transform');
}

/** Interaction or a preference change consumes the one-time entrance immediately. */
function finish(session: Session) {
  if (session.disposed) return;
  session.played = true;
  session.observer?.disconnect();
  settle(session);
}

/** Late callbacks cannot recreate resources after their owner unmounts. */
function dispose(session: Session) {
  if (session.disposed) return;
  finish(session);
  session.disposed = true;
  session.cleanup.forEach((release) => release());
}

/** Listeners are registered with their matching release operation. */
function listen(session: Session, target: HTMLElement, type: string, callback: () => void) {
  target.addEventListener(type, callback);
  session.cleanup.push(() => target.removeEventListener(type, callback));
}

/** Focus and pointer input settle animation before a user acts on its controls. */
function interactions(session: Session) {
  for (const type of MOTION_INTERACTIONS) listen(session, session.element, type, () => finish(session));
  const changed = () => {
    if (session.preference.matches) finish(session);
  };
  session.preference.addEventListener('change', changed);
  session.cleanup.push(() => session.preference.removeEventListener('change', changed));
}

/** The keyframe shape stays small while each preset owns its named distances. */
type Preset = (typeof MOTION_PRESETS)[RevealPreset];
/** Compositor-only properties leave document geometry untouched. */
function keyframes(config: Preset, mobile: boolean, connector: boolean) {
  return {
    x: [mobile && config.x ? MOTION_MOBILE_DISTANCE : config.x, 0],
    y: [mobile && config.y ? MOTION_MOBILE_DISTANCE : config.y, 0],
    scale: [mobile && config.scale !== 1 ? MOTION_MOBILE_SCALE : config.scale, 1],
    opacity: [config.opacity, 1],
    scaleY: [connector ? 0 : 1, 1],
  };
}
/** Mobile distances are chosen when playback starts, rather than at an earlier viewport. */
function frames(preset: RevealPreset) {
  return keyframes(MOTION_PRESETS[preset], window.matchMedia(MOTION_MOBILE_QUERY).matches, preset === 'connector');
}

/** The named presets keep duration and stagger bounded. */
function start(session: Session, preset: RevealPreset, order: number) {
  const delay = Math.min(Math.max(order, 0), MOTION_MAX_ORDER) * MOTION_STAGGER;
  session.control = animate(session.element, frames(preset), {
    duration: MOTION_PRESETS[preset].duration,
    ease: MOTION_EASE,
    delay,
  });
}

/** Each animation is finite; setup failure returns the region to its readable final state. */
function play(session: Session, preset: RevealPreset, order = 0) {
  if (session.disposed || session.preference.matches) return finish(session);
  settle(session);
  try {
    start(session, preset, order);
  } catch {
    finish(session);
  }
}

/** The observer's initial notification may be visible but still below the requested threshold. */
const crossedThreshold = (entries: IntersectionObserverEntry[]) =>
  entries.some((entry) => entry.isIntersecting && entry.intersectionRatio >= MOTION_THRESHOLD);

/** An actual threshold crossing consumes observation before playback starts. */
function enter(session: Session, entries: IntersectionObserverEntry[], preset: RevealPreset, order: number) {
  if (session.disposed || session.played || !crossedThreshold(entries)) return;
  session.played = true;
  session.observer?.disconnect();
  play(session, preset, order);
}

/** Partial setup is torn down through the same path as normal route cleanup. */
function connect(session: Session, setup: () => void) {
  try {
    interactions(session);
    setup();
  } catch {
    dispose(session);
  }
  return () => dispose(session);
}

/** A reveal has no hidden initial state and never repeats when a reader scrolls back. */
export function attachReveal(element: HTMLElement, preset: RevealPreset, order = 0): () => void {
  const session = createSession(element);
  if (!shouldReveal(session)) return noop;
  return connect(session, () => {
    session.observer = new IntersectionObserver((entries) => enter(session, entries, preset, order), {
      threshold: MOTION_THRESHOLD,
    });
    session.observer.observe(element);
  });
}

/** Focus can arrive in server HTML before hydration, so listeners alone cannot establish an untouched region. */
function shouldReveal(session: Session | null): session is Session {
  return (
    !!session &&
    !session.preference.matches &&
    typeof IntersectionObserver === 'function' &&
    !session.element.contains(session.element.ownerDocument.activeElement)
  );
}

/** Opening a native disclosure animates only its body; closing and its keyboard semantics remain native. */
export function attachDisclosure(details: HTMLDetailsElement, body: HTMLElement): () => void {
  const session = createSession(body);
  if (!session) return noop;
  return connect(session, () =>
    listen(session, details, 'toggle', () => {
      if (details.open) play(session, 'disclosure');
      else finish(session);
    }),
  );
}
