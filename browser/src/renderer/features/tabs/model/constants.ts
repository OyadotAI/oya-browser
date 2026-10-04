/**
 * The tab strip's fixed words and keys. Its sizes and timings are in
 * core/constants.ts (TAB_*), next to the CSS values they match.
 */

/** The key that closes the focused tab. */
export const CLOSE_KEY = 'Delete';

/** The key that cancels a drag. */
export const CANCEL_KEY = 'Escape';

/** Pointer buttons: the primary one presses and drags, the middle one closes. */
export const BUTTONS = { primary: 0, middle: 1 } as const;

/** What the strip and its hover card say. */
export const TEXT = {
  /** An untitled tab. */
  untitled: 'New tab',
  /** The close button's name, before the title. */
  close: 'Close ',
  /** The close button's name for an untitled tab. */
  closeUntitled: 'Close tab',
  /** The card's address line on the start page. */
  home: 'Oya start page',
  /** The card's address line for a tab with no address. */
  blank: 'Blank page',
  /** The strip's names. */
  bar: 'Browser tabs',
  list: 'Open pages',
  newTab: 'New tab',
  newTabTitle: 'New tab (⌘/Ctrl T)',
} as const;

/** The scheme the hover card leaves off an address. */
export const WEB_SCHEME = /^https?:\/\//;
