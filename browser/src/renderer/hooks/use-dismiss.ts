/**
 * Closing a popover (a menu, a dropdown list) the way people expect: a press
 * outside it, or Escape. Page-wide listeners, held only while it is open.
 */
import { useEffect } from 'react';

/** Calls `close` on a press whose target `inside` does not claim, while `open`. */
export function useClickOutside(open: boolean, inside: (target: Node) => boolean, close: () => void): void {
  useEffect(() => {
    if (!open) return;
    const press = (event: MouseEvent) => {
      if (!(event.target instanceof Node) || !inside(event.target)) close();
    };
    document.addEventListener('mousedown', press);
    return () => document.removeEventListener('mousedown', press);
  }, [open, inside, close]);
}

/** Calls `close` on Escape, while `open`. */
export function useEscapeKey(open: boolean, close: () => void): void {
  useEffect(() => {
    if (!open) return;
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape') close();
    };
    document.addEventListener('keydown', key);
    return () => document.removeEventListener('keydown', key);
  }, [open, close]);
}
