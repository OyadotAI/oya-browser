/** Route-owned navigation disclosures; listeners disappear when the header unmounts. */
'use client';
import { useCallback, useEffect, useState } from 'react';
import type { KeyboardEvent, RefObject } from 'react';

/** The layout breakpoint shared with the navigation stylesheet. */
const DESKTOP = '(min-width: 1024px)';

/** Resets disclosures on layout changes without leaving document state behind. */
function useLayoutReset(close: () => void) {
  useEffect(() => {
    if (!window.matchMedia) return;
    const query = window.matchMedia(DESKTOP);
    query.addEventListener('change', close);
    return () => query.removeEventListener('change', close);
  }, [close]);
}

/** A revealed mobile menu starts its keyboard journey at the first destination. */
function useMenuFocus(open: boolean) {
  useEffect(() => {
    if (open) document.getElementById('main-links')?.querySelector<HTMLElement>('a, button')?.focus();
  }, [open]);
}

/** Closes open disclosures on pointer taps outside the header without stealing focus. */
function useOutsideClose(open: boolean, headerRef: RefObject<HTMLElement | null>, close: () => void) {
  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!headerRef.current?.contains(event.target as Node)) close();
    };
    document.addEventListener('pointerdown', onPointerDown);
    return () => document.removeEventListener('pointerdown', onPointerDown);
  }, [open, headerRef, close]);
}

/** Owns the two disclosures and returns focus to the control that opened them. */
export function useNavigation(headerRef: RefObject<HTMLElement | null>) {
  const [state, setState] = useState({ menu: false, product: false });
  const close = useCallback(() => setState({ menu: false, product: false }), []);
  useLayoutReset(close);
  useMenuFocus(state.menu);
  useOutsideClose(state.menu || state.product, headerRef, close);
  const toggle = (key: keyof typeof state) => setState((current) => ({ ...current, [key]: !current[key] }));
  const escape = (event: KeyboardEvent<HTMLElement>) => dismiss(event, state, close);
  return { state, close, toggle, escape };
}

/** The two disclosure flags owned by the header. */
interface NavState {
  /** Whether the mobile menu is open. */
  menu: boolean;
  /** Whether the Product panel is open. */
  product: boolean;
}

/** Closes a disclosure on Escape while keeping keyboard navigation anchored to its trigger. */
function dismiss(event: KeyboardEvent<HTMLElement>, state: NavState, close: () => void) {
  if (event.key !== 'Escape' || (!state.menu && !state.product)) return;
  event.preventDefault();
  const id = state.menu ? 'main-links' : 'product-links';
  const trigger = event.currentTarget.querySelector<HTMLButtonElement>(`[aria-controls="${id}"]`);
  close();
  trigger?.focus();
}
