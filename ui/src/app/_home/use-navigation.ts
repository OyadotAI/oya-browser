/** Route-owned navigation disclosures; listeners disappear when the header unmounts. */
'use client';
import { useCallback, useEffect, useState } from 'react';
import type { KeyboardEvent } from 'react';

/** The layout breakpoint shared with the navigation stylesheet. */
const DESKTOP = '(min-width: 1024px)';

/** Resets disclosures on layout changes without leaving document state behind. */
function useLayoutReset(close: () => void) {
  useEffect(() => {
    const query = window.matchMedia(DESKTOP);
    query.addEventListener('change', close);
    return () => query.removeEventListener('change', close);
  }, [close]);
}

/** A revealed mobile menu starts its keyboard journey at the first destination. */
function useMenuFocus(open: boolean) {
  useEffect(() => {
    if (open) document.getElementById('main-links')?.querySelector<HTMLAnchorElement>('a')?.focus();
  }, [open]);
}

/** Owns the two disclosures and returns focus to the control that opened them. */
export function useNavigation() {
  const [state, setState] = useState({ menu: false, product: false });
  const close = useCallback(() => setState({ menu: false, product: false }), []);
  useLayoutReset(close);
  useMenuFocus(state.menu);
  const toggle = (key: keyof typeof state) => setState((current) => ({ ...current, [key]: !current[key] }));
  const escape = (event: KeyboardEvent<HTMLElement>) => dismiss(event, state.menu, close);
  return { state, close, toggle, escape };
}

/** Closes a disclosure on Escape while keeping keyboard navigation anchored to its trigger. */
function dismiss(event: KeyboardEvent<HTMLElement>, menuOpen: boolean, close: () => void) {
  if (event.key !== 'Escape') return;
  event.preventDefault();
  const id = menuOpen ? 'main-links' : 'product-links';
  const trigger = event.currentTarget.querySelector<HTMLButtonElement>(`[aria-controls="${id}"]`);
  close();
  trigger?.focus();
}
