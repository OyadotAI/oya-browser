/**
 * The dialog's behavior: focus moves in and is trapped there, Escape closes
 * the topmost dialog, the page stops scrolling, and focus returns to whatever
 * opened it on close.
 */
'use client';

import { useEffect, useEffectEvent, useRef } from 'react';

/** What can take focus inside a dialog. */
const FOCUSABLE =
  'a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])';

/** Open dialogs holding the scroll lock; the page scrolls again when the last one closes. */
let scrollLocks = 0;
/** The body's overflow before the first lock, restored after the last. */
let originalOverflow = '';

/** Stops the page scrolling behind a dialog. */
function lockScroll() {
  if (scrollLocks++ === 0) originalOverflow = document.body.style.overflow;
  document.body.style.overflow = 'hidden';
}

/** Releases one lock; the last one restores the page's scrolling. */
function unlockScroll() {
  if (--scrollLocks === 0) document.body.style.overflow = originalOverflow;
}

/** The dialog's visible focusable elements, in order. */
function focusables(node: HTMLElement): HTMLElement[] {
  return [...node.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.offsetParent !== null);
}

/** Where Tab should jump to wrap around the dialog, or null to let it move normally. */
function wrapTarget(e: KeyboardEvent, node: HTMLElement, items: HTMLElement[]): HTMLElement | null {
  const outside = !node.contains(document.activeElement);
  if (e.shiftKey) return document.activeElement === items[0] || outside ? items[items.length - 1] : null;
  return document.activeElement === items[items.length - 1] || outside ? items[0] : null;
}

/** Keeps Tab and Shift+Tab cycling inside `node`. */
function trapTab(e: KeyboardEvent, node: HTMLElement) {
  const items = focusables(node);
  if (!items.length) return e.preventDefault();
  const target = wrapTarget(e, node, items);
  if (!target) return;
  e.preventDefault();
  target.focus();
}

/** A keydown while the dialog is open: only the topmost dialog answers. */
function onDialogKey(e: KeyboardEvent, node: HTMLElement | null, close: () => void) {
  const dialogs = document.querySelectorAll('[role="dialog"]');
  if (dialogs[dialogs.length - 1] !== node) return;
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopImmediatePropagation();
    return close();
  }
  if (e.key === 'Tab' && node) trapTab(e, node);
}

/** Opens the dialog's side effects and returns their undo. */
function openDialog(node: HTMLDivElement | null, close: () => void) {
  const opener = document.activeElement as HTMLElement | null;
  // First focusable, or the panel itself so keys still land inside.
  (node?.querySelector<HTMLElement>(FOCUSABLE) ?? node)?.focus({ preventScroll: true });
  const onKey = (e: KeyboardEvent) => onDialogKey(e, node, close);
  document.addEventListener('keydown', onKey, true);
  lockScroll();
  return () => closeDialog(onKey, opener);
}

/** Undoes openDialog: listener off, scroll back, focus back to the opener. */
function closeDialog(onKey: (e: KeyboardEvent) => void, opener: HTMLElement | null) {
  document.removeEventListener('keydown', onKey, true);
  unlockScroll();
  if (opener?.isConnected) opener.focus({ preventScroll: true });
}

/** The panel ref for a dialog that is `open`; closing calls `onClose`. */
export function useDialog(open: boolean, onClose: () => void) {
  const panel = useRef<HTMLDivElement>(null);
  const close = useEffectEvent(onClose);
  useEffect(() => (open ? openDialog(panel.current, () => close()) : undefined), [open]);
  return panel;
}
