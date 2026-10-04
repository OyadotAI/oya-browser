/** Limits and fixed choices for what the shell may ask of the main process (src/main/ipc/). */

/** The largest Playwright script the shell may ask to save, in characters. */
export const MAX_EXPORT_CHARS = 2_000_000;
/** The largest workflow file that may be opened, in bytes. */
export const MAX_IMPORT_BYTES = 5_000_000;
/** The index of the confirming button in a two-button question. */
export const CONFIRM_BUTTON = 1;
/** Themes the shell may choose. */
export const THEMES: readonly string[] = ['system', 'light', 'dark'];
/** A name safe to suggest as a file name: letters, digits, '_' and '-', at most 64 of them. */
export const SAFE_FILE_NAME = /^[\w-]{1,64}$/;
