/**
 * Appearance: system, light or dark, following the OS when set to system, and
 * the saved UI preferences (theme, platform, last pane) at start.
 */
/* global oyaBrowser, Dom, DevPanel */
/* exported Theme */

/** The shell's theme. */
const Theme = {
  /** The chosen appearance: system, light or dark. */
  theme: 'system',
  /** Whether the OS is in dark mode. */
  systemDark: matchMedia('(prefers-color-scheme: dark)').matches,

  /** Applies the theme to the document, all at once: controls must not fade from one theme to the other. */
  apply() {
    const root = document.documentElement;
    root.dataset.themeSwitching = 'true';
    root.dataset.theme = Theme.theme === 'system' ? (Theme.systemDark ? 'dark' : 'light') : Theme.theme;
    requestAnimationFrame(() => requestAnimationFrame(() => delete root.dataset.themeSwitching));
  },

  /** The OS appearance changed. */
  systemChanged(dark) {
    Theme.systemDark = dark;
    Theme.apply();
  },

  /** The person picked an appearance. */
  choose() {
    Theme.theme = Dom.byId('theme-preference').value;
    Theme.apply();
    oyaBrowser.saveUiPreferences({ theme: Theme.theme });
  },

  /** Restores the saved preferences, and the pane that was open. */
  restore(value) {
    Theme.theme = ['system', 'light', 'dark'].includes(value.theme) ? value.theme : 'system';
    if (typeof value.systemDark === 'boolean') Theme.systemDark = value.systemDark;
    Dom.byId('theme-preference').value = Theme.theme;
    document.documentElement.dataset.platform = value.platform;
    Theme.apply();
    if (['record', 'chat', 'actions', 'network', 'source'].includes(value.pane)) DevPanel.show(value.pane);
  },
};

oyaBrowser.onShellAppearance(Theme.systemChanged);
Dom.byId('theme-preference').addEventListener('change', Theme.choose);
Theme.apply();
oyaBrowser.getUiPreferences().then(Theme.restore);
