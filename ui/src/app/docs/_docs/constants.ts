/**
 * Every number the docs page runs on, by name.
 */

/** A heading this close to the top of the viewport (or above it) is the current section. */
export const ACTIVE_HEADING_OFFSET_PX = 140;
/** Wait before scrolling after a sidebar click, so a closing mobile menu does not fight the scroll. */
export const NAV_SCROLL_DELAY_MS = 100;
/** Pause in typing before the search runs. */
export const SEARCH_DEBOUNCE_MS = 150;
/** Shorter queries show no results. */
export const MIN_QUERY_LENGTH = 2;
/** Index entries shorter than this are not searchable. */
export const MIN_ENTRY_LENGTH = 3;
/** Characters of an entry that identify it when removing duplicates. */
export const DEDUPE_PREFIX_LENGTH = 40;
/** Characters of context shown each side of a match. */
export const SNIPPET_CONTEXT = 30;
/** Most results shown. */
export const MAX_HITS = 12;
/** How long "Copied" shows on a code block. */
export const COPIED_RESET_MS = 1800;
