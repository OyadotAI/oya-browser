/**
 * What the shell page's composition root (app/shell-root.tsx) builds once and
 * hands to the features: the bridge and the shared ViewModels. A feature's
 * ViewModel takes the slice it uses, `Pick<RendererServices, 'bridge' | 'shell'>`,
 * plus small interfaces for other features it calls (wired by the root).
 */
import type { OyaBrowser } from '../core/bridge.ts';
import type { ShellViewModel } from './shell-view-model.ts';
import type { PanelViewModel, FrameClock } from './panel/panel-view-model.ts';

/** The shared services of the shell page. */
export interface RendererServices {
  /** The main process (window.oyaBrowser). */
  bridge: OyaBrowser;
  /** Connected, mode, start page, recording. */
  shell: ShellViewModel;
  /** The workspace panel: open, pane, layout, Clear. */
  panel: PanelViewModel;
  /** Animation frames (requestAnimationFrame in the page, by hand in tests). */
  frames: FrameClock;
}
