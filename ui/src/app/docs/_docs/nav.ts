/**
 * Moving around the docs: the current section (from the scroll position),
 * the navigate() every link uses, and the context that shares them.
 */
'use client';

import { createContext, useCallback, useContext, useEffect, useState } from 'react';
import { ACTIVE_HEADING_OFFSET_PX, NAV_SCROLL_DELAY_MS } from './constants';

/** The section in view and how to go to another. */
export interface DocsNavValue {
  /** The id of the section in view. */
  active: string;
  /** Goes to a section by id. */
  navigate: (id: string) => void;
}

/** Shared by the sidebar links and the in-page links. */
export const DocsNav = createContext<DocsNavValue>({ active: 'control-plane', navigate: () => {} });

/** The docs navigation, for any component on the page. */
export const useDocsNav = () => useContext(DocsNav);

/** Scrolls a section into view, if it exists. */
export function scrollToSection(id: string) {
  document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
}

/** The last heading at or above the activation line. */
function currentHeading(): HTMLElement | undefined {
  const headings = [...document.querySelectorAll<HTMLElement>('main h2[id], main h3[id]')];
  return headings.filter((el) => el.getBoundingClientRect().top <= ACTIVE_HEADING_OFFSET_PX).at(-1);
}

/** Reports the current heading, if any. */
function markCurrent(setActive: (id: string) => void) {
  const current = currentHeading();
  if (current) setActive(current.id);
}

/** Follows the scroll, once per frame, and reports the current section; returns the stop. */
function trackHeadings(setActive: (id: string) => void) {
  let frame = 0;
  const update = () => {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => markCurrent(setActive));
  };
  window.addEventListener('scroll', update, { passive: true });
  return () => (window.removeEventListener('scroll', update), cancelAnimationFrame(frame));
}

/** Goes to a section: closes the mobile menu, marks it active, updates the URL, then scrolls. */
function goTo(id: string, setActive: (id: string) => void, closeMenu: () => void) {
  closeMenu();
  setActive(id);
  window.history.replaceState(null, '', '#' + id);
  setTimeout(() => scrollToSection(id), NAV_SCROLL_DELAY_MS);
}

/** The section in view, and a navigate() that also closes the mobile menu. */
export function useDocsNavState(closeMenu: () => void): DocsNavValue {
  const [active, setActive] = useState('control-plane');
  useEffect(() => trackHeadings(setActive), []);
  const navigate = useCallback((id: string) => goTo(id, setActive, closeMenu), [closeMenu]);
  return { active, navigate };
}

/** "/" focuses the visible search box (desktop or mobile), unless an input already has focus. */
function focusSearchOnSlash(e: KeyboardEvent) {
  if (e.key !== '/' || (document.activeElement as HTMLElement)?.tagName === 'INPUT') return;
  e.preventDefault();
  const boxes = [...document.querySelectorAll<HTMLInputElement>('[aria-label="Search documentation"]')];
  boxes.find((el) => el.getClientRects().length)?.focus();
}

/** Binds "/" to the search box while the page is open. */
export function useSlashToSearch() {
  useEffect(() => {
    document.addEventListener('keydown', focusSearchOnSlash);
    return () => document.removeEventListener('keydown', focusSearchOnSlash);
  }, []);
}
