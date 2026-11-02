/**
 * Unit tests for the head theme script: a saved choice wins, otherwise the
 * page follows the OS at load and on every change, telling the toggles.
 */
import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { THEME_SCRIPT } from '@/lib/theme-script';

/** The OS theme the stubbed matchMedia reports, and the change handler it was given. */
let osDark = false;
let onChange: (() => void) | undefined;

/** Runs the script the way the browser would: as a plain inline statement. */
const run = () => new Function(THEME_SCRIPT)();
/** Flips the fake OS theme and fires the listener the script registered. */
const flipOs = () => {
  osDark = !osDark;
  onChange?.();
};

beforeEach(() => {
  osDark = false;
  onChange = undefined;
  vi.stubGlobal('matchMedia', () => ({
    get matches() {
      return osDark;
    },
    addEventListener: (_: string, fn: () => void) => {
      onChange = fn;
    },
  }));
});

afterEach(() => {
  vi.unstubAllGlobals();
  delete document.documentElement.dataset.theme;
  localStorage.clear();
});

describe('THEME_SCRIPT', () => {
  it('follows a light OS at first paint when nothing is saved', () => {
    run();
    expect(document.documentElement.dataset.theme).toBe('light');
  });

  it('lets a saved theme beat the OS', () => {
    localStorage.setItem('oya_theme', 'dark');
    run();
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('ignores an unrecognised saved value and follows the OS', () => {
    localStorage.setItem('oya_theme', 'blue');
    run();
    expect(document.documentElement.dataset.theme).toBe('light');
  });

  it('flips the page on an OS change and tells every toggle', () => {
    const heard = vi.fn();
    window.addEventListener('oya-theme', heard);
    run();
    flipOs();
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(heard).toHaveBeenCalledTimes(1);
    window.removeEventListener('oya-theme', heard);
  });

  it('keeps a choice made after load when the OS changes later', () => {
    run();
    localStorage.setItem('oya_theme', 'light');
    flipOs();
    expect(document.documentElement.dataset.theme).toBe('light');
  });

  it('follows the OS at load and on change when storage is blocked', () => {
    vi.stubGlobal('localStorage', {
      getItem: () => {
        throw new Error('blocked');
      },
    });
    run();
    expect(document.documentElement.dataset.theme).toBe('light');
    flipOs();
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('leaves the server default alone when matchMedia is missing', () => {
    vi.stubGlobal('matchMedia', undefined);
    document.documentElement.dataset.theme = 'dark';
    expect(run).not.toThrow();
    expect(document.documentElement.dataset.theme).toBe('dark');
  });
});
