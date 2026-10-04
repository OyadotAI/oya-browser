/**
 * Every number human-like input runs on, by name: key codes, modifier bits,
 * and the timing ranges that make typing and mouse movement look like a hand.
 * A range is `base + Math.random() * spread` milliseconds.
 */

/** A key as CDP's Input.dispatchKeyEvent describes it. */
export interface KeyDef {
  /** The DOM `key` value. */
  key: string;
  /** The DOM `code` value. */
  code: string;
  /** The Windows virtual key code. */
  keyCode: number;
  /** The character it types, if it types one. */
  text?: string;
  /** Whether typing it needs Shift. */
  shift?: boolean;
}

/** A random duration: `base + Math.random() * spread` milliseconds. */
export interface Range {
  /** The shortest it can be. */
  base: number;
  /** How much longer it can be. */
  spread: number;
}

/** Named keys in CDP Input.dispatchKeyEvent format. */
export const KEY_DEFS: Readonly<Record<string, KeyDef>> = {
  Enter: { key: 'Enter', code: 'Enter', keyCode: 13 },
  Tab: { key: 'Tab', code: 'Tab', keyCode: 9 },
  Backspace: { key: 'Backspace', code: 'Backspace', keyCode: 8 },
  Delete: { key: 'Delete', code: 'Delete', keyCode: 46 },
  Escape: { key: 'Escape', code: 'Escape', keyCode: 27 },
  ArrowUp: { key: 'ArrowUp', code: 'ArrowUp', keyCode: 38 },
  ArrowDown: { key: 'ArrowDown', code: 'ArrowDown', keyCode: 40 },
  ArrowLeft: { key: 'ArrowLeft', code: 'ArrowLeft', keyCode: 37 },
  ArrowRight: { key: 'ArrowRight', code: 'ArrowRight', keyCode: 39 },
  Home: { key: 'Home', code: 'Home', keyCode: 36 },
  End: { key: 'End', code: 'End', keyCode: 35 },
  PageUp: { key: 'PageUp', code: 'PageUp', keyCode: 33 },
  PageDown: { key: 'PageDown', code: 'PageDown', keyCode: 34 },
  Space: { key: ' ', code: 'Space', keyCode: 32, text: ' ' },
};

/** CDP modifier bits. */
export const Modifier = {
  /** Ctrl, the select-all modifier off macOS. */
  CTRL: 2,
  /** Cmd, the select-all modifier on macOS. */
  META: 4,
  /** Shift, for upper-case characters. */
  SHIFT: 8,
} as const;

/** Base keystroke gap: 8-26ms (~200-300 WPM). */
export const TYPE_BASE: Range = { base: 8, spread: 18 };
/** Extra pause after a space: a small word-boundary pause. */
export const WORD_PAUSE: Range = { base: 5, spread: 15 };
/** Extra pause before a special character. */
export const SPECIAL_PAUSE: Range = { base: 5, spread: 10 };
/** A micro-hesitation, added to this share of keystrokes. */
export const HESITATION: Range = { base: 30, spread: 50 };
/** Share of keystrokes that hesitate (2%). */
export const HESITATION_CHANCE = 0.02;
/** How long a pressed key is held. */
export const KEY_HOLD: Range = { base: 20, spread: 30 };
/** Pauses around clearing a field. */
export const CLEAR_PAUSE: Range = { base: 10, spread: 15 };
/** Hover before a click, and hold before its release. */
export const CLICK_PAUSE: Range = { base: 10, spread: 20 };
/** Gap between mouse-path points. */
export const MOVE_PAUSE: Range = { base: 4, spread: 12 };

/** Fewest points on a mouse path. */
export const MIN_PATH_STEPS = 5;
/** Most points on a mouse path. */
export const MAX_PATH_STEPS = 30;
/** Pixels of travel per path point. */
export const PX_PER_PATH_STEP = 25;
/** How far control points stray, as a share of the distance. */
export const PATH_JITTER = 0.3;
/** The second control point strays less than the first. */
export const SECOND_POINT_JITTER = 0.6;
/** Where along the line the first control point sits. */
export const FIRST_POINT_AT = 0.25;
/** Where along the line the second control point sits. */
export const SECOND_POINT_AT = 0.75;
/** Centres Math.random() on zero. */
export const RANDOM_CENTRE = 0.5;
/** Ease-out exponent: fast at the start, slowing near the target. */
export const EASE_POWER = 2;
/** The Bernstein coefficient of a cubic Bézier's inner terms. */
export const CUBIC_INNER_WEIGHT = 3;
