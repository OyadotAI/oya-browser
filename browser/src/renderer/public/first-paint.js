/**
 * Runs before the shell's first paint, from the page's head: puts the theme the
 * main process chose (in the page's address) on the root, so the launch stage and
 * everything under it paint in the right theme at once, with no light flash in the
 * dark. features/chrome/theme-view-model.ts takes over when the saved preferences arrive.
 */

/** The theme the address carries, if it is one the shell draws. */
const FIRST_THEME = new URLSearchParams(location.search).get('theme');
if (FIRST_THEME === 'dark' || FIRST_THEME === 'light') document.documentElement.dataset.theme = FIRST_THEME;

/** Shell-only motion policy is established before styles or application code run. */
document.documentElement.dataset.oyaStill = String(new URLSearchParams(location.search).get('still') === 'true');

/** An incognito window is marked before its first paint, so it never flashes the normal chrome (core/tokens.css). */
document.documentElement.dataset.incognito = String(new URLSearchParams(location.search).get('incognito') === 'true');

/** Whether this browser offers incognito windows at all; governance is fixed at launch, so this never changes. */
document.documentElement.dataset.incognitoAvailable = String(
  new URLSearchParams(location.search).get('incognitoAvailable') === 'true',
);

/** A detached or new window is already part of a running app; it never replays launch. */
document.documentElement.dataset.secondary = String(new URLSearchParams(location.search).get('secondary') === 'true');
