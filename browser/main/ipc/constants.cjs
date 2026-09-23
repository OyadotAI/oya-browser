/** Limits and fixed choices for what the shell may ask of the main process (main/ipc/). */

/** The largest Playwright script the shell may ask to save, in characters. */
const MAX_EXPORT_CHARS = 2_000_000;
/** The largest workflow file that may be opened, in bytes. */
const MAX_IMPORT_BYTES = 5_000_000;
/** The index of the confirming button in a two-button question. */
const CONFIRM_BUTTON = 1;
/** Themes the shell may choose. */
const THEMES = ['system', 'light', 'dark'];
/** Dev-panel panes the shell may remember. */
const PANES = ['record', 'chat', 'routines', 'actions', 'network', 'source'];

module.exports = { MAX_EXPORT_CHARS, MAX_IMPORT_BYTES, CONFIRM_BUTTON, THEMES, PANES };
