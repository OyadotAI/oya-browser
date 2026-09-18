/**
 * Where the right-click menu is open, and for which row.
 */
import { useState, type MouseEvent } from 'react';
import type { BrowserRow } from '../types';

/** An open menu: the cursor position and the row it acts on. */
export interface OpenMenu {
  /** Viewport position of the right-click. */
  at: {
    /** Pixels from the viewport's left edge. */
    x: number;
    /** Pixels from the viewport's top edge. */
    y: number;
  };
  /** The row the menu acts on. */
  row: BrowserRow;
}

/** `openAt` is the row's contextmenu handler; `close` dismisses the menu. */
export function useRowMenu() {
  const [menu, setMenu] = useState<OpenMenu | null>(null);
  const openAt = (e: MouseEvent, row: BrowserRow) => {
    e.preventDefault();
    setMenu({ at: { x: e.clientX, y: e.clientY }, row });
  };
  return { menu, openAt, close: () => setMenu(null) };
}
