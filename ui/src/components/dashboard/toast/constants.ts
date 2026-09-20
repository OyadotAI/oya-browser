/**
 * Every number the toasts run on, by name.
 */

/** How long a toast stays before it dismisses itself. */
export const TOAST_LIFETIME_MS = 3000;

/** Where a toast enters from and leaves to: slightly above and smaller. */
export const TOAST_HIDDEN = { opacity: 0, y: -8, scale: 0.96 };
/** A toast at rest. */
export const TOAST_SHOWN = { opacity: 1, y: 0, scale: 1 };
/** Enter and exit animation. */
export const TOAST_TRANSITION = { duration: 0.2 };
