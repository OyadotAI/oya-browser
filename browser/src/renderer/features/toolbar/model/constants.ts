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

/** How long a saved or cancelled download stays on the chip while the person can see it. */
export const CHIP_DONE_MS = 8000;
/** How long a newly saved file is on the chip before Open and Show act on it, so a page cannot swap one in under a click. */
export const CHIP_ARM_MS = 500;
/** How long a failed or moved download stays: longer, so it is read, but never all day. */
export const CHIP_FAILED_MS = 30000;

/** The Downloads button and its chip's words (the inline download indicator). */
export const DOWNLOAD_TEXT = {
  /** The button's label, idle. */
  button: 'Downloads',
  /** Its tooltip's shortcut. */
  keys: ' (⌘ Shift J, Ctrl J)',
  /** The status word, by phase. */
  running: 'Downloading',
  saved: 'Saved',
  failed: 'Failed',
  cancelled: 'Cancelled',
  gone: 'Moved or deleted',
  /** More than one running. */
  files: (n: number) => `Downloading ${n} files`,
  /** The two actions on a saved file. */
  open: 'Open',
  show: (mac: boolean) => (mac ? 'Show in Finder' : 'Show in folder'),
  showShort: 'Show',
  openLabel: (name: string) => `Open ${name}`,
  showLabel: (name: string, mac: boolean) => `Show ${name} in ${mac ? 'Finder' : 'folder'}`,
  /** The body's accessible name before ". Show all downloads", by phase. */
  describe: {
    running: (name: string, bytes: string) => `Downloading ${name}, ${bytes}`,
    files: (n: number) => `Downloading ${n} files`,
    saved: (name: string, size: string) => `Saved ${name}, ${size}`,
    failed: (name: string) => `${name} didn't download`,
    cancelled: (name: string) => `${name} was cancelled, nothing was saved`,
    gone: (name: string) => `${name} was moved or deleted`,
  },
  /** Added to the body's accessible name: what clicking it does. */
  showAll: '. Show all downloads',
  /** Bytes so far: with the size, or without it when the site did not say. */
  of: (received: string, total: string) => `${received} of ${total}`,
  soFar: (received: string) => `${received} so far`,
  /** The chip's meter: how much has arrived of how much, and the percent. */
  meter: (received: string, total: string, percent: number) => `${received} of ${total} · ${percent}%`,
  /** What a screen reader hears, politely, as a download moves on. */
  say: {
    running: (name: string) => `Downloading ${name}`,
    saved: (name: string) => `Saved ${name}`,
    failed: (name: string) => `${name} didn't download`,
    cancelled: (name: string) => `${name} was cancelled`,
    gone: () => 'That file was moved or deleted.',
  },
} as const;

/** A whole, in percent. */
export const PERCENT = 100;
/** The download ring's box, in SVG units (drawn at 14px). */
export const RING_SIZE = 14;
/** Its radius, leaving room for the 2px stroke. */
export const RING_RADIUS = 6;
/** The ring's centre. */
export const RING_CENTRE = 7;
/** The ring's circumference (2π × its radius), which its dash is measured against. */
export const RING_LENGTH = 37.7;
/** The share of the ring drawn, turning, when a site gave no size. */
export const RING_INDETERMINATE = 0.25;
