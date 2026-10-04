/**
 * What the root keeps in step on <html> and <body>, outside any feature's
 * markup: the welcome-or-browsing mode class, the panel's layout as CSS
 * variables, whether the window is hidden (so loops pause), and the ready mark
 * the Electron tests wait for.
 */
import { useEffect } from 'react';
import { useViewModel } from '../hooks/index.ts';
import type { ShellViewModel } from './shell-view-model.ts';
import type { PanelViewModel } from './panel/panel-view-model.ts';
import type { ShellLayout } from '../../shared/ipc.ts';

/** body.mode-setup or body.mode-browsing, following the shell's mode. */
function useModeClass(shell: ShellViewModel): void {
  const { mode } = useViewModel(shell);
  useEffect(() => {
    document.body.classList.toggle('mode-setup', mode === 'setup');
    document.body.classList.toggle('mode-browsing', mode === 'browsing');
  }, [mode]);
}

/** Puts a layout on the root as CSS variables and the panel-moving flag. */
function applyLayout(layout: ShellLayout): void {
  const root = document.documentElement;
  const px = (n: number) => n + 'px';
  root.style.setProperty('--panel-reveal', px(layout.reveal));
  root.style.setProperty('--panel-width', px(layout.panelWidth));
  root.style.setProperty('--panel-height', px(layout.panelHeight));
  root.style.setProperty('--chrome-height', px(layout.chromeHeight));
  root.dataset.panelMoving = String(layout.progress > 0 && layout.progress < 1);
}

/** The panel's layout on the root, as the main process computes it. */
function useLayoutVariables(panel: PanelViewModel): void {
  const { layout } = useViewModel(panel);
  useEffect(() => void (layout && applyLayout(layout)), [layout]);
}

/** html[data-hidden] while the window is hidden, so the orb's loops pause. */
function useVisibilityMark(): void {
  useEffect(() => {
    const mark = () => document.documentElement.setAttribute('data-hidden', String(document.hidden));
    document.addEventListener('visibilitychange', mark);
    mark();
    return () => document.removeEventListener('visibilitychange', mark);
  }, []);
}

/** html[data-ready] once the shell has rendered: the Electron tests wait for it. */
function useReadyMark(): void {
  useEffect(() => void (document.documentElement.dataset.ready = 'true'), []);
}

/** Every page-wide effect the root owns. */
export function useRootEffects(shell: ShellViewModel, panel: PanelViewModel): void {
  useModeClass(shell);
  useLayoutVariables(panel);
  useVisibilityMark();
  useReadyMark();
}
