/**
 * Keeps focus in the full-screen workbench while it is open, as a dialog must:
 * the console behind it is made inert (no Tab into it, no clicks, hidden from
 * assistive tech), focus moves into the workbench, and goes back to whatever
 * opened it on close. Escape is left to the workbench's own keys, so the
 * editor's search box can take it first.
 */
import { useEffect, useRef } from 'react';

/** Makes every other child of the body inert; answers the undo. */
function inertOthers(keep: HTMLElement | null): () => void {
  const others = [...document.body.children].filter((el) => el !== keep && !el.hasAttribute('inert'));
  others.forEach((el) => el.setAttribute('inert', ''));
  return () => others.forEach((el) => el.removeAttribute('inert'));
}

/** Holds focus in `root`; answers the undo, which gives focus back to what had it. */
function holdFocus(root: HTMLElement | null): () => void {
  const opener = document.activeElement as HTMLElement | null;
  const restore = inertOthers(root);
  root?.focus({ preventScroll: true });
  return () => {
    restore();
    if (opener?.isConnected) opener.focus({ preventScroll: true });
  };
}

/** The workbench's root ref; while mounted, focus stays inside it. */
export function useWorkbenchFocus() {
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => holdFocus(root.current), []);
  return root;
}
