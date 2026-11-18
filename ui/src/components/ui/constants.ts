/** Finite interface motion shares one cadence and uses smaller travel on narrow screens. */
const MOTION_EASE_START = 0.22;
/** The exit control keeps related animations on one curve. */
const MOTION_EASE_FINISH = 0.36;
/** Shared easing settles quickly without a spring overshoot. */
export const MOTION_EASE: [number, number, number, number] = [MOTION_EASE_START, 1, MOTION_EASE_FINISH, 1];
/** Regions enter once a meaningful portion is visible. */
export const MOTION_THRESHOLD = 0.2;
/** Stagger remains short even when a caller supplies many items. */
export const MOTION_MAX_ORDER = 4;
/** Seconds between related visual elements. */
export const MOTION_STAGGER = 0.07;
/** Narrow layouts use a smaller distance without changing document geometry. */
export const MOTION_MOBILE_QUERY = '(max-width: 767px)';
/** A live preference change must settle motion immediately. */
export const MOTION_REDUCED_QUERY = '(prefers-reduced-motion: reduce)';
/** The mobile entrance moves only a few pixels. */
export const MOTION_MOBILE_DISTANCE = 6;
/** Mobile artwork avoids a pronounced zoom. */
export const MOTION_MOBILE_SCALE = 0.98;
/** Interacting with a region makes its final position immediately available. */
export const MOTION_INTERACTIONS = ['focusin', 'pointerdown'];
/** The visible-content presets keep text readable throughout playback. */
export const MOTION_PRESETS = {
  intro: { x: 0, y: 12, scale: 1, opacity: 0.85, duration: 0.65 },
  section: { x: 0, y: 12, scale: 1, opacity: 0.85, duration: 0.55 },
  artwork: { x: 40, y: 0, scale: 0.94, opacity: 0.35, duration: 0.9 },
  branch: { x: 0, y: 8, scale: 1, opacity: 0.85, duration: 0.65 },
  document: { x: 0, y: 12, scale: 0.98, opacity: 0.85, duration: 0.6 },
  public: { x: 0, y: 8, scale: 1, opacity: 0.85, duration: 0.35 },
  connector: { x: 0, y: 0, scale: 1, opacity: 1, duration: 0.65 },
  disclosure: { x: 0, y: 4, scale: 1, opacity: 0.85, duration: 0.18 },
};
/** A finite set prevents page-specific motion styles from drifting. */
export type RevealPreset = keyof typeof MOTION_PRESETS;
