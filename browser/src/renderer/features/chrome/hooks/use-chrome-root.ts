/**
 * The attributes the stylesheets key the whole shell on, set from ViewModel
 * state by the composition root: html[data-theme], [data-theme-switching]
 * and [data-platform] from the theme, html[data-agent-active] from the
 * agent's activity, and body.on-home from the shell (the start page sets it).
 * Also how the root reads the first paint's appearance.
 */
import { useEffect } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import type { ShellViewModel } from '../../../app/shell-view-model.ts';
import { appliedTheme, type ThemeState, type ThemeViewModel } from '../view-models/theme-view-model.ts';
import type { AgentActivityViewModel } from '../view-models/activity-view-model.ts';

/** What the root effect reads. */
export interface ChromeRoot {
  /** The theme. */
  theme: ThemeViewModel;
  /** The agent's activity. */
  activity: AgentActivityViewModel;
  /** The shell (onHome). */
  shell: ShellViewModel;
}

/** Puts the theme on the root element. */
function applyTheme(root: HTMLElement, state: ThemeState): void {
  root.dataset.theme = appliedTheme(state);
  if (state.platform) root.dataset.platform = state.platform;
  if (state.switching) root.dataset.themeSwitching = 'true';
  else delete root.dataset.themeSwitching;
}

/** Keeps html and body in step with the theme, the agent's activity and the start page. */
export function useChromeRoot({ theme, activity, shell }: ChromeRoot): void {
  const themeState = useViewModel(theme);
  const { active } = useViewModel(activity);
  const { onHome } = useViewModel(shell);
  useEffect(() => void applyTheme(document.documentElement, themeState), [themeState]);
  useEffect(() => void (document.documentElement.dataset.agentActive = String(active)), [active]);
  useEffect(() => void document.body.classList.toggle('on-home', onHome), [onHome]);
}

/** Whether the OS was dark at the first paint: html[data-theme] (core/first-paint.js), else the media query. */
export function systemDarkAtStart(): boolean {
  const painted = document.documentElement.dataset.theme;
  return painted ? painted === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches;
}
