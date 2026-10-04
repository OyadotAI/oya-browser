/**
 * The shared hooks' fixed values: what a dialog's focus trap moves between,
 * and the keys that move focus along a row of tabs.
 */

/** Controls the dialog focus trap moves between. */
export const FOCUSABLE = 'button:not(:disabled), input, select, textarea, summary, [tabindex="0"]';

/** Keys that move focus along a row (and resize the panel from its handle). */
export const ROVING_KEYS = ['ArrowLeft', 'ArrowRight', 'Home', 'End'] as const;

/** A key that moves focus along a row. */
export type RovingKey = (typeof ROVING_KEYS)[number];

/** The media query for a person who asked for less motion. */
export const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)';
