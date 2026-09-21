/**
 * The context menu's behavior: where it sits, which item is active, and the
 * keys and outside clicks it answers.
 */
'use client';

import { useEffect, useEffectEvent, useLayoutEffect, useRef, useState, type RefObject } from 'react';
import type { MenuItem } from './context-menu';

/** Gap kept between the menu and the viewport edge, in pixels. */
const EDGE_MARGIN_PX = 8;

/** A point on screen. */
export interface Point {
  /** Pixels from the left. */
  x: number;
  /** Pixels from the top. */
  y: number;
}

/** What a menu key needs to act. */
interface MenuState {
  /** The menu's items. */
  items: MenuItem[];
  /** The active item's index. */
  active: number;
  /** Moves the active item. */
  setActive: (i: number) => void;
  /** Closes the menu. */
  onClose: () => void;
}

/** The point clamped so a menu of `size` stays fully on screen. */
function clampToViewport(at: Point, size: DOMRect): Point {
  return {
    x: Math.max(EDGE_MARGIN_PX, Math.min(at.x, window.innerWidth - size.width - EDGE_MARGIN_PX)),
    y: Math.max(EDGE_MARGIN_PX, Math.min(at.y, window.innerHeight - size.height - EDGE_MARGIN_PX)),
  };
}

/** Moves the active item `step` places among the enabled ones, wrapping around. */
function move(m: MenuState, step: 1 | -1) {
  const enabled = m.items.map((it, i) => (it.disabled ? -1 : i)).filter((i) => i >= 0);
  if (!enabled.length) return;
  const cur = enabled.indexOf(m.active);
  m.setActive(enabled[(cur + step + enabled.length) % enabled.length]);
}

/** Picks the active item, unless it is disabled. */
function pick(m: MenuState) {
  const item = m.items[m.active];
  if (!item || item.disabled) return;
  item.onSelect();
  m.onClose();
}

/** What each key does while the menu is open. */
const MENU_KEYS: Record<string, (m: MenuState, e: KeyboardEvent) => void> = {
  Escape: (m, e) => {
    e.stopImmediatePropagation();
    m.onClose();
  },
  ArrowDown: (m) => move(m, 1),
  ArrowUp: (m) => move(m, -1),
  Enter: pick,
};

/** Runs the key's action, if it has one. */
function handleMenuKey(m: MenuState, e: KeyboardEvent) {
  if (!Object.hasOwn(MENU_KEYS, e.key)) return;
  e.preventDefault();
  MENU_KEYS[e.key](m, e);
}

/** Places the menu on screen when it opens and activates the first enabled item. */
function usePlacement(at: Point | null, items: MenuItem[], ref: RefObject<HTMLDivElement | null>) {
  const [pos, setPos] = useState(at);
  const [active, setActive] = useState(0);
  const firstEnabled = useEffectEvent(() => items.findIndex((i) => !i.disabled));
  useLayoutEffect(() => {
    if (at && ref.current) setPos(clampToViewport(at, ref.current.getBoundingClientRect()));
    if (at && ref.current) setActive(firstEnabled());
  }, [at, ref]);
  return { pos, active, setActive };
}

/** While open: outside clicks close it, keys are captured, and it takes focus. */
function useOpenListeners(at: Point | null, ref: RefObject<HTMLDivElement | null>, m: MenuState) {
  const closeOutside = useEffectEvent((e: MouseEvent) => !ref.current?.contains(e.target as Node) && m.onClose());
  const onKey = useEffectEvent((e: KeyboardEvent) => handleMenuKey(m, e));
  useEffect(() => {
    if (!at) return;
    return listen(ref.current, closeOutside, onKey);
  }, [at, ref]);
}

/** Adds the document listeners and focuses the menu; returns their removal. */
function listen(menu: HTMLDivElement | null, close: (e: MouseEvent) => void, key: (e: KeyboardEvent) => void) {
  const onDown = (e: MouseEvent) => close(e);
  const onKey = (e: KeyboardEvent) => key(e);
  document.addEventListener('mousedown', onDown);
  document.addEventListener('keydown', onKey, true);
  menu?.focus({ preventScroll: true });
  return () => unlisten(onDown, onKey);
}

/** Removes what listen() added. */
function unlisten(onDown: (e: MouseEvent) => void, onKey: (e: KeyboardEvent) => void) {
  document.removeEventListener('mousedown', onDown);
  document.removeEventListener('keydown', onKey, true);
}

/**
 * A right-click menu's state. Opens at the cursor, stays on screen, arrows
 * move, Enter picks, Escape and any click outside close it.
 */
export function useContextMenu(at: Point | null, items: MenuItem[], onClose: () => void) {
  const ref = useRef<HTMLDivElement>(null);
  const { pos, active, setActive } = usePlacement(at, items, ref);
  useOpenListeners(at, ref, { items, active, setActive, onClose });
  return { ref, pos, active, setActive };
}
