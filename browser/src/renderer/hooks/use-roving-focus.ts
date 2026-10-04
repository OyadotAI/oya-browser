/**
 * Roving focus along a row of tabs: the arrow keys move to the next or
 * previous tab (wrapping round), Home and End to the first and last. The
 * index arithmetic is pure, so a ViewModel can use it without the DOM.
 */
import type { KeyboardEvent } from 'react';
import { ROVING_KEYS, type RovingKey } from './constants.ts';

/** Whether `key` moves focus along a row. */
export const isRovingKey = (key: string): key is RovingKey => (ROVING_KEYS as readonly string[]).includes(key);

/** The index `key` moves to from `current` among `count` items, wrapping round. */
export function rovingIndex(key: RovingKey, current: number, count: number): number {
  if (key === 'Home') return 0;
  if (key === 'End') return count - 1;
  return (current + (key === 'ArrowRight' ? 1 : count - 1)) % count;
}

/**
 * A tablist's keydown handler: moves focus from the focused `[role="tab"]` to
 * the one the key names, after telling `onMove` its index.
 */
export function useRovingFocus(onMove: (index: number) => void) {
  return (event: KeyboardEvent<HTMLElement>): void => {
    if (isRovingKey(event.key)) moveFocus(event, event.key, onMove);
  };
}

/** Moves focus to the tab `key` names, if a tab of the list has focus now. */
function moveFocus(event: KeyboardEvent<HTMLElement>, key: RovingKey, onMove: (index: number) => void): void {
  const items = [...event.currentTarget.querySelectorAll<HTMLElement>('[role="tab"]')];
  const current = items.indexOf(document.activeElement as HTMLElement);
  if (current < 0) return;
  event.preventDefault();
  const index = rovingIndex(key, current, items.length);
  onMove(index);
  items[index].focus();
}
