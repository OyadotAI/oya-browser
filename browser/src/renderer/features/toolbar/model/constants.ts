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
} as const;

/** The key that sends the address bar. */
export const SUBMIT_KEY = 'Enter';

/** A dedicated overlay prevents native page views from covering the suggestions. */
export const ADDRESS_OVERLAY = 'address';

/** What the translate chip says: `{lang}` is the page's language and `{target}` the person's, both by name. */
export const TRANSLATE_TEXT = {
  offer: 'Translate from {lang}',
  offerTitle: "Translate this page into {target} with your project's AI model",
  translating: 'Translating…',
  translatingTitle: 'Translating from {lang}. Click to stop and show the original.',
  translated: 'Translated from {lang} · Show original',
  translatedTo: 'Translated to {target} · Show original',
  partial: 'Translated the first part · Show original',
  partialTitle:
    'This page is longer than Oya translates at once. The first part is in {target}; the rest is unchanged.',
  signIn: 'Sign in to translate',
  signInTitle: "Sign in to Oya to translate pages with your project's AI model.",
  connect: 'Connect a model to translate',
  connectTitle: 'Connect an AI model to translate pages. It is the same model Assistant uses.',
  notAllowed: 'Model not allowed',
  keyRefused: 'Model key refused',
  payment: 'Provider needs payment',
  unavailable: "Model didn't answer",
  limit: 'Hourly limit reached',
  limitTitle:
    "Translation and Assistant share your project's hourly model allowance, and it is used up. Try again later, or connect your own key in Assistant.",
  rate: 'Too many requests',
  rateTitle: 'Too many model requests in the last minute. Wait a moment, then click to try again.',
  claim: 'Free steps used up',
  offline: "Couldn't reach Oya",
  offlineTitle: "Oya's server can't be reached. Check your connection, then click to try again.",
  timeout: 'Translation timed out',
  timeoutTitle: 'The translation took too long and was stopped. Click to try again.',
  empty: 'Nothing to translate',
  emptyTitle: "This page has no text Oya can translate. Text inside images, PDFs and frames isn't covered.",
  failed: "Couldn't translate",
  malformedTitle: "The model's answer couldn't be used, so the page was left as it was. Click to try again.",
  /** What a screen reader hears as the chip changes; never when it first offers. */
  saidStart: 'Translating from {lang}',
  saidDone: 'Translated from {lang}',
  saidPartial: 'Translated the first part of this page',
  saidOriginal: 'Showing the original page',
} as const;
