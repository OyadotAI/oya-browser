/**
 * Runs before the shell's first paint, from the page's head: puts the theme the
 * main process chose (in the page's address) on the root, so the launch stage and
 * everything under it paint in the right theme at once, with no light flash in the
 * dark. features/chrome/theme-view-model.ts takes over when the saved preferences arrive.
 */

/** The theme the address carries, if it is one the shell draws. */
const FIRST_THEME = new URLSearchParams(location.search).get('theme');
if (FIRST_THEME === 'dark' || FIRST_THEME === 'light') document.documentElement.dataset.theme = FIRST_THEME;
