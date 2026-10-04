/**
 * Closes a routine's open menu on a press outside it (or its ⋯ button), or on
 * Escape.
 */
import { useClickOutside, useEscapeKey } from '../../../hooks/index.ts';

/** What a press inside the menu or on its button looks like. */
const MENU_PARTS = '.routine-menu, .routine-more';

/** Whether a press landed in the menu or on its button. */
const inMenu = (target: Node): boolean => target instanceof Element && !!target.closest(MENU_PARTS);

/** Calls `close` on a press outside the menu, or on Escape, while `open`. */
export function useMenuDismiss(open: boolean, close: () => void): void {
  useClickOutside(open, inMenu, close);
  useEscapeKey(open, close);
}
