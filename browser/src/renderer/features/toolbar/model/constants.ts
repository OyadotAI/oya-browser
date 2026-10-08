/**
 * The navigation toolbar's words: button names, the address bar's status,
 * the window title, and the Ask button's titles.
 */

/** What the toolbar says. */
export const TEXT = {
  /** The toolbar's name. */
  toolbar: 'Browser navigation',
  library: 'Library',
  libraryTitle: 'History and bookmarks (⌘/Ctrl Y)',
  back: 'Back',
  forward: 'Forward',
  reload: 'Reload page',
  stop: 'Stop loading',
  /** The address bar's name and placeholder. */
  address: 'Search or enter URL',
  /** The address bar's status while the page loads, and after it failed. */
  loading: 'Loading…',
  failed: 'Load failed',
  /** The window's title, alone and after a page's title. */
  app: 'Oya Browser',
  titleJoin: ', ',
  /** The Ask button's label and titles. */
  ask: 'Oya',
  askTitle: 'Ask Oya (⌘/Ctrl Shift D)',
  askRecordingTitle: 'Oya Agent · recording',
} as const;

/** The key that sends the address bar. */
export const SUBMIT_KEY = 'Enter';
