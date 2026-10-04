/**
 * A modal dialog's keyboard and focus: while it is open, focus starts on one
 * control, Escape closes it, and Tab and Shift+Tab wrap around its visible
 * controls; when it closes, focus goes back where it was.
 */
import { useEffect, type RefObject } from 'react';
import { FOCUSABLE } from './constants.ts';

/** The dialog's visible, focusable controls, in tab order. */
function focusable(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.getClientRects().length > 0);
}

/** Keeps Tab focus within `root`'s visible controls. */
function trapTab(event: KeyboardEvent, root: HTMLElement): void {
  const items = focusable(root);
  const [first, last] = [items[0], items[items.length - 1]];
  const edge = event.shiftKey ? first : last;
  if (!edge || document.activeElement !== edge) return;
  event.preventDefault();
  (event.shiftKey ? last : first).focus();
}

/** Escape closes; Tab wraps. */
function dialogKey(event: KeyboardEvent, root: HTMLElement | null, close: () => void): void {
  if (event.key === 'Escape') {
    event.preventDefault();
    close();
  } else if (event.key === 'Tab' && root) trapTab(event, root);
}

/** Puts focus back where it was before the dialog opened, once it closes. */
function useRestoreFocus(open: boolean): void {
  useEffect(() => {
    if (!open) return;
    const last = document.activeElement;
    return () => {
      if (last instanceof HTMLElement && last.isConnected) last.focus();
    };
  }, [open]);
}

/** Makes the element in `ref` a modal dialog while `open`, focusing the element with id `focusId` (it changes with the page). */
export function useDialog(ref: RefObject<HTMLElement | null>, open: boolean, close: () => void, focusId: string): void {
  useRestoreFocus(open);
  useEffect(() => {
    if (open) document.getElementById(focusId)?.focus();
  }, [open, focusId]);
  useDialogKeys(ref, open, close);
}

/** Escape and Tab, while the dialog is open. */
function useDialogKeys(ref: RefObject<HTMLElement | null>, open: boolean, close: () => void): void {
  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => dialogKey(event, ref.current, close);
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [open, close, ref]);
}
