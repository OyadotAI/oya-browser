/**
 * Where the page sits in the window: below the toolbar, beside (or, when the
 * window is narrow, above) the dev panel. The panel slides open and shut, and
 * its width is remembered.
 */
import type { BrowserView } from 'electron';
import type { AppServices } from '../app/services.ts';
import { shellLayout } from './shell-layout.ts';
import {
  PANEL_WIDTH,
  PANEL_MIN_WIDTH,
  PANEL_MAX_WIDTH,
  PANEL_SAVE_DELAY_MS,
  PANEL_MOTION_MS,
  PANEL_FRAME_MS,
  PANEL_EASE_POWER,
} from './constants.ts';

/** The services the layout uses. */
type Deps = Pick<AppServices, 'tabs' | 'shell' | 'shield' | 'config'>;

/** The dev panel's state and the active tab's bounds. */
export class PanelLayout {
  /** Whether the dev panel is open (or opening). */
  open = false;
  /** The panel's width, as the person last dragged it. */
  width: number = PANEL_WIDTH;
  /** How far open the panel is drawn, 0 to 1. */
  progress = 0;
  /** The slide animation, while it runs. */
  private motionTimer: NodeJS.Timeout | undefined = undefined;
  /** A pending save of the panel width. */
  private saveTimer: NodeJS.Timeout | undefined = undefined;
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` is the main-process context (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Lays out the shell at `progress` (a number mid-slide; anything else means settled). */
  layoutActiveTab(progress?: number): void {
    if (typeof progress !== 'number') {
      clearInterval(this.motionTimer);
      progress = this.open ? 1 : 0;
    }
    this.progress = progress;
    const view = this.deps.tabs.getActiveView() as BrowserView | null; // a tab shown in the window is a BrowserView; popups have windows of their own
    if (this.deps.shell.window) this.apply(view, progress);
  }

  /** Tells the shell the layout, moves the page, and keeps the shield over it. */
  private apply(view: BrowserView | null, progress: number): void {
    const bounds = this.deps.shell.window!.getContentBounds(); // layoutActiveTab checked it
    const layout = shellLayout(bounds.width, bounds.height, progress, this.width);
    this.deps.shell.send('shell-layout', { ...layout, panelOpen: this.open });
    if (view && this.deps.shell.browsingMode) view.setBounds(layout.page);
    this.deps.shield.sync();
  }

  /** Opens or closes the panel, sliding unless the person prefers reduced motion. */
  toggle(reducedMotion = false): boolean {
    this.open = !this.open;
    clearInterval(this.motionTimer);
    const from = this.progress;
    if (reducedMotion) this.layoutActiveTab();
    else this.slide(from, this.open ? 1 : 0);
    this.deps.shell.send('dev-panel-state', this.open);
    return this.open;
  }

  /** Animates from one openness to another. */
  private slide(from: number, target: number): void {
    const started = performance.now();
    const tick = (): void => this.frame(from, target, started);
    this.motionTimer = setInterval(tick, PANEL_FRAME_MS);
    tick();
  }

  /** One frame of the slide, eased out. */
  private frame(from: number, target: number, started: number): void {
    if (!this.deps.shell.alive()) return clearInterval(this.motionTimer);
    const elapsed = Math.min(1, (performance.now() - started) / PANEL_MOTION_MS);
    const eased = 1 - Math.pow(1 - elapsed, PANEL_EASE_POWER);
    this.layoutActiveTab(from + (target - from) * eased);
    if (elapsed === 1) clearInterval(this.motionTimer);
  }

  /** Opens the panel without a slide, for a result that should be seen now. */
  reveal(): void {
    if (this.open) return;
    this.open = true;
    this.layoutActiveTab();
    this.deps.shell.send('dev-panel-state', this.open);
  }

  /** The person dragged the panel edge: clamp, remember, relayout. */
  resize(width: number): number {
    if (!Number.isFinite(width)) return this.width;
    this.width = Math.max(PANEL_MIN_WIDTH, Math.min(width, PANEL_MAX_WIDTH));
    this.deps.config.values.ui = { ...this.deps.config.values.ui, panelWidth: this.width };
    this.scheduleSave();
    this.layoutActiveTab();
    return this.width;
  }

  /** Saves the width once the drag settles. */
  private scheduleSave(): void {
    clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => {
      this.saveTimer = undefined;
      this.deps.config.save();
    }, PANEL_SAVE_DELAY_MS);
  }

  /** On quit: stop the slide and write any unsaved width now. */
  flush(): void {
    clearInterval(this.motionTimer);
    if (!this.saveTimer) return;
    clearTimeout(this.saveTimer);
    this.deps.config.save();
  }
}
