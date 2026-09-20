/**
 * Keyboard shortcuts for the console: one document listener that matches
 * bindings like 'mod+1' or 'shift+?', and how a binding is drawn on key caps.
 */
'use client';

import { useEffect } from 'react';

/** One keyboard binding. */
export interface Shortcut {
  /** e.g. 'n', '/', 'Escape', 'mod+1', 'shift+?' — `mod` is ⌘ on Mac and Ctrl elsewhere. */
  keys: string;
  /** What it does, for the help sheet. */
  label: string;
  /** Group shown in the help sheet. */
  group: 'Fleet' | 'Browser' | 'Navigate';
  /** Fire even while an input is focused (Escape usually wants this). */
  global?: boolean;
  /** Runs when the binding matches. */
  handler: (e: KeyboardEvent) => void;
}

/** Whether `mod` means ⌘ (Mac) or Ctrl (everything else). */
const IS_MAC = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);

/** Binding names whose KeyboardEvent.key differs from the name. */
const KEY_NAMES: Record<string, string> = { up: 'arrowup', down: 'arrowdown' };

/** Elements whose keys belong to the field, not the console. */
const EDITABLE = /^(input|textarea|select)$/i;

/** While one of these is open it owns the keyboard. */
const OVERLAY = '[role="dialog"], [role="menu"]';

/** How each binding part reads on a key cap. */
const CAPS: Record<string, string> = {
  mod: IS_MAC ? '⌘' : 'Ctrl',
  shift: '⇧',
  alt: IS_MAC ? '⌥' : 'Alt',
  escape: 'Esc',
  Escape: 'Esc',
  enter: '↵',
  Enter: '↵',
  up: '↑',
  down: '↓',
};

/** Whether the event's modifiers are exactly the ones the binding asks for. */
function modifiersMatch(e: KeyboardEvent, parts: string[], key: string): boolean {
  const mod = IS_MAC ? e.metaKey : e.ctrlKey;
  if (parts.includes('mod') !== mod) return false;
  if (parts.includes('alt') !== e.altKey) return false;
  // '?' already implies shift on most layouts; do not require it twice.
  return !(parts.includes('shift') && !e.shiftKey && key !== '?');
}

/** Whether a keydown is the binding `keys`. */
function matches(e: KeyboardEvent, keys: string): boolean {
  const parts = keys.toLowerCase().split('+');
  const key = parts.pop()!;
  if (!modifiersMatch(e, parts, key)) return false;
  const wanted = Object.hasOwn(KEY_NAMES, key) ? KEY_NAMES[key] : key;
  return e.key.toLowerCase() === wanted;
}

/** Whether the key was typed into a text field. */
function inField(target: HTMLElement | null): boolean {
  return !!target && (EDITABLE.test(target.tagName) || target.isContentEditable);
}

/** Runs the first binding a keydown matches, unless something else owns the key. */
function dispatch(e: KeyboardEvent, shortcuts: Shortcut[]) {
  if (e.defaultPrevented || document.querySelector(OVERLAY)) return;
  const target = e.target as HTMLElement | null;
  // While the live view holds the keyboard, every key is the browser's —
  // including Escape, which it uses to hand the keyboard back.
  if (target?.closest?.('[data-captures-keys]')) return;
  const hit = shortcuts.find((s) => matches(e, s.keys));
  if (!hit || (inField(target) && !hit.global)) return;
  e.preventDefault();
  hit.handler(e);
}

/**
 * One listener for the whole console. Keys typed into a field stay in the
 * field unless the binding says `global`; the live view marks itself with
 * data-captures-keys so its own keyboard handling wins while it has focus.
 */
export function useShortcuts(shortcuts: Shortcut[], enabled = true) {
  useEffect(() => {
    if (!enabled) return;
    const onKey = (e: KeyboardEvent) => dispatch(e, shortcuts);
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [shortcuts, enabled]);
}

/** How a binding reads on a key cap: 'mod+1' → ['⌘', '1'] or ['Ctrl', '1']. */
export function keyCaps(keys: string): string[] {
  return keys.split('+').map((k) => (Object.hasOwn(CAPS, k) ? CAPS[k] : k.length === 1 ? k.toUpperCase() : k));
}
