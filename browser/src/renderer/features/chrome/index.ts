/**
 * The window chrome's entry: the brand mark (the tab bar's `brand` slot),
 * the appearance picker (the shell dialog's footer), the page backdrop, the
 * launch stage, their ViewModels, and the root effect the composition root
 * calls once (`useChromeRoot`), with the first-paint readers it builds the
 * theme and launch from.
 */
export { BrandMark, ThemeSelect, PageBackdrop, type ChromeProps } from './components/chrome-views.tsx';
export { Launch, type LaunchProps } from './components/launch.tsx';
export { ThemeViewModel, appliedTheme, type ThemeDeps, type ThemeState } from './view-models/theme-view-model.ts';
export { AgentActivityViewModel, type ActivityState } from './view-models/activity-view-model.ts';
export { BackdropViewModel, type BackdropDeps, type BackdropState } from './view-models/backdrop-view-model.ts';
export { LaunchViewModel, type LaunchState } from './view-models/launch-view-model.ts';
export { useChromeRoot, systemDarkAtStart, type ChromeRoot } from './hooks/use-chrome-root.ts';
