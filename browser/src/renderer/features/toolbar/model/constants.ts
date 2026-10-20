/**
 * The navigation toolbar's words: button names, the address bar's status,
 * the window title, and the Ask button's titles.
 */

/** What the toolbar says. */
export const TEXT = {
  /** The toolbar's name. */
  toolbar: 'Browser navigation',
  library: 'History',
  /** The bookmarks bar's name. */
  bookmarks: 'Bookmarks',
  /** What the bar says before any bookmark. */
  bookmarksEmpty: 'Bookmark a page to keep it here.',
  /** The bar's way to every bookmark and the history. */
  allBookmarks: 'Bookmarks',
  /** The star's name when the page is not saved. */
  bookmarkPage: 'Bookmark this page',
  /** The star's name when it is. */
  unbookmarkPage: 'Remove bookmark',
  /** The bar's hide button. */
  hideBookmarks: 'Hide bookmarks bar',
  libraryTitle: 'History (⌘/Ctrl Y)',
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
  /** The window's name in an incognito window. */
  appIncognito: 'Oya Browser (Incognito)',
  /** The toolbar's incognito badge, where Ask sits in a normal window. */
  incognito: 'Incognito',
  /** The Ask button's label and titles. */
  ask: 'Assistant',
  askTitle: 'Open Assistant (⌘/Ctrl Shift D)',
  askRecordingTitle: 'Oya Agent · recording',
  /** The find bar's name, and its field's placeholder. */
  find: 'Find in page',
  /** The find bar's count when the page has no match. */
  findNone: 'No results',
  /** Between the selected match and the total ("3 of 17"). */
  findOf: ' of ',
  /** The find bar's buttons; the previous and next names get their shortcut appended. */
  findPrevious: 'Previous match',
  findNext: 'Next match',
  findClose: 'Close find bar (Esc)',
} as const;

/** The key that sends the address bar. */
export const SUBMIT_KEY = 'Enter';

/** A dedicated overlay prevents native page views from covering the suggestions. */
export const ADDRESS_OVERLAY = 'address';

/** A page at actual size, as a percentage: the zoom chip hides at it. */
export const FULL_SIZE_PERCENT = 100;

/** What the address bar's zoom chip says, for a page at `percent`. */
export const ZOOM_TEXT = {
  /** The chip itself: "110%". */
  chip: (percent: number) => `${percent}%`,
  /** Its tooltip. */
  title: (percent: number) => `Zoomed to ${percent}%. Click to reset to 100% (⌘/Ctrl 0)`,
  /** Its accessible name. */
  label: (percent: number) => `Zoom ${percent}%, reset to 100%`,
  /** What a screen reader hears when a person zooms. */
  announce: (percent: number) => `Zoom ${percent}%`,
  /** What it hears when the page is back to actual size. */
  reset: 'Zoom reset to 100%',
} as const;

/** What the share chip says. `shared` is the shared tab's host, `call` the calling tab's. */
export const SHARE_TEXT = {
  /** The shared tab when it is not a website. */
  oyaPage: 'an Oya page',
  /** The calling tab when it is not a website. */
  theCall: 'the call',
  /** The chip's action. */
  action: 'Go to call',
  /** The chip's status text. */
  status: (shared: string) => `Sharing ${shared}`,
  /** The chip's accessible name. */
  label: (shared: string, call: string) => `Go to call on ${call}. You're sharing ${shared}.`,
  /** The chip's tooltip: why it does not simply share this tab instead. */
  title: (shared: string, call: string) =>
    `You're sharing ${shared} with ${call}. Oya can't move a live share to another tab, so this takes you to the call. Stop sharing there, then share again and pick the tab you want.`,
} as const;
