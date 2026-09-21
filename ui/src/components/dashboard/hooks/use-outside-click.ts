/**
 * Closes a popover when the user presses the mouse anywhere outside it.
 */
import { useEffect, type RefObject } from 'react';

/** Calls `onOutside` on every mousedown outside `ref`. Pass a stable callback. */
export function useOutsideClick(ref: RefObject<HTMLElement | null>, onOutside: () => void) {
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onOutside();
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [ref, onOutside]);
}
