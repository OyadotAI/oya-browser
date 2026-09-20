/**
 * Keys inside the switcher popover: Escape closes it, and the arrow keys
 * move between projects on the list.
 */
import type { KeyboardEvent } from 'react';
import { close } from './actions';
import type { PickerContext } from './types';

/** Moves focus to the next or previous project, wrapping at the ends. */
function moveFocus(e: KeyboardEvent<HTMLDivElement>) {
  const options = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>('[data-project-option]'));
  if (!options.length) return;
  e.preventDefault();
  const index = options.indexOf(document.activeElement as HTMLButtonElement);
  options[(index + (e.key === 'ArrowDown' ? 1 : -1) + options.length) % options.length]?.focus();
}

/** The popover's keydown handler. Arrows only navigate on the project list. */
export function onPickerKeyDown(c: PickerContext, e: KeyboardEvent<HTMLDivElement>) {
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    close(c);
  }
  if (!c.ui.form && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) moveFocus(e);
}
