/**
 * The workbench's keys, as VS Code binds them: ⌘S saves, ⌘J shows or hides the
 * test panel, ⌘B the explorer, and Escape asks to close. Ctrl stands in for ⌘
 * off a Mac. Bound on the window while the workbench is open.
 */
import { useEffect, useRef } from 'react';

/** What each key does. */
export interface WorkbenchKeys {
  /** ⌘S. */
  save: () => void;
  /** ⌘J. */
  togglePanel: () => void;
  /** ⌘B. */
  toggleSidebar: () => void;
  /** Escape. */
  close: () => void;
}

/** A key press → the action it runs, or undefined. */
function actionOf(e: KeyboardEvent, keys: WorkbenchKeys): (() => void) | undefined {
  // The editor already used it (Escape closing its search, ⌘S saving).
  if (e.defaultPrevented) return undefined;
  if (e.key === 'Escape') return keys.close;
  if (!(e.metaKey || e.ctrlKey)) return undefined;
  const byKey: Record<string, () => void> = { s: keys.save, j: keys.togglePanel, b: keys.toggleSidebar };
  const key = e.key.toLowerCase();
  return Object.hasOwn(byKey, key) ? byKey[key] : undefined;
}

/** Runs the action a key press names, if any, instead of the browser's own. */
function handle(e: KeyboardEvent, keys: WorkbenchKeys) {
  const run = actionOf(e, keys);
  if (!run) return;
  e.preventDefault();
  run();
}

/** Binds the keys on the window, always calling the latest actions. */
export function useWorkbenchKeys(keys: WorkbenchKeys) {
  const latest = useRef(keys);
  useEffect(() => void (latest.current = keys));
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => handle(e, latest.current);
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
}
