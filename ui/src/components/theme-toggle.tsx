/**
 * The light/dark switch. The theme lives on <html data-theme>, is remembered
 * in localStorage, and every toggle on the page follows an `oya-theme` event.
 */
'use client';

import { Moon, Sun } from 'lucide-react';
import { useSyncExternalStore } from 'react';

/** Listens for theme changes from any toggle. */
const subscribe = (callback: () => void) => {
  window.addEventListener('oya-theme', callback);
  return () => window.removeEventListener('oya-theme', callback);
};
/** Whether the page is light right now. */
const snapshot = () => document.documentElement.dataset.theme === 'light';

/** A button that flips the theme. */
export default function ThemeToggle() {
  const light = useSyncExternalStore(subscribe, snapshot, () => false);
  return (
    <button
      className="btn-icon"
      aria-label={`Switch to ${light ? 'dark' : 'light'} theme`}
      title={`Switch to ${light ? 'dark' : 'light'} theme`}
      onClick={() => {
        const theme = light ? 'dark' : 'light';
        document.documentElement.dataset.theme = theme;
        try {
          localStorage.setItem('oya_theme', theme);
        } catch {
          /* private mode */
        }
        window.dispatchEvent(new Event('oya-theme'));
      }}
    >
      {light ? <Sun size={16} /> : <Moon size={16} />}
    </button>
  );
}
