/**
 * Whether the person asked for less motion. Read once where a ViewModel is
 * built (the launch stage, the tab strip's slides), so it is a plain
 * function rather than a hook.
 */
import { REDUCED_MOTION_QUERY } from './constants.ts';

/** True when the system asks for reduced motion. */
export const reducedMotion = (): boolean => !!window.matchMedia?.(REDUCED_MOTION_QUERY).matches;
