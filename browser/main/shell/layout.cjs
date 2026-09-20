/**
 * Where the page sits in the window: below the toolbar, beside (or, when the
 * window is narrow, above) the dev panel. The panel slides open and shut, and
 * its width is remembered.
 */
const { shellLayout } = require('../../shell-layout.cjs');
const constants = require('./constants.cjs');

/** The dev panel's state and the active tab's bounds. */
class PanelLayout {
  /** `ctx` is the main-process context (see main.js). */
  constructor(ctx) {
    /** The main-process context. */
    this.ctx = ctx;
    /** Whether the dev panel is open (or opening). */
    this.open = false;
    /** The panel's width, as the person last dragged it. */
    this.width = constants.DEFAULT_PANEL_WIDTH;
    /** How far open the panel is drawn, 0 to 1. */
    this.progress = 0;
    /** The slide animation, while it runs. */
    this.motionTimer = undefined;
    /** A pending save of the panel width. */
    this.saveTimer = undefined;
  }

  /** Lays out the shell at `progress` (a number mid-slide; anything else means settled). */
  layoutActiveTab(progress) {
    if (typeof progress !== 'number') {
      clearInterval(this.motionTimer);
      progress = this.open ? 1 : 0;
    }
    this.progress = progress;
    const view = this.ctx.tabs.getActiveView();
    if (this.ctx.shell.window) this.apply(view, progress);
  }

  /** Tells the shell the layout, moves the page, and keeps the shield over it. */
  apply(view, progress) {
    const bounds = this.ctx.shell.window.getContentBounds();
    const layout = shellLayout(bounds.width, bounds.height, progress, this.width);
    this.ctx.shell.send('shell-layout', layout);
    if (view && this.ctx.shell.browsingMode) view.setBounds(layout.page);
    this.ctx.shield.sync();
  }

  /** Opens or closes the panel, sliding unless the person prefers reduced motion. */
  toggle(reducedMotion = false) {
    this.open = !this.open;
    clearInterval(this.motionTimer);
    const from = this.progress;
    if (reducedMotion) this.layoutActiveTab();
    else this.slide(from, this.open ? 1 : 0);
    this.ctx.shell.send('dev-panel-state', this.open);
    return this.open;
  }

  /** Animates from one openness to another. */
  slide(from, target) {
    const started = performance.now();
    const tick = () => this.frame(from, target, started);
    this.motionTimer = setInterval(tick, constants.PANEL_FRAME_MS);
    tick();
  }

  /** One frame of the slide, eased out. */
  frame(from, target, started) {
    if (!this.ctx.shell.alive()) return clearInterval(this.motionTimer);
    const elapsed = Math.min(1, (performance.now() - started) / constants.PANEL_MOTION_MS);
    const eased = 1 - Math.pow(1 - elapsed, constants.PANEL_EASE_POWER);
    this.layoutActiveTab(from + (target - from) * eased);
    if (elapsed === 1) clearInterval(this.motionTimer);
  }

  /** Opens the panel without a slide, for a result that should be seen now. */
  reveal() {
    if (this.open) return;
    this.open = true;
    this.layoutActiveTab();
    this.ctx.shell.send('dev-panel-state', this.open);
  }

  /** The person dragged the panel edge: clamp, remember, relayout. */
  resize(width) {
    if (!Number.isFinite(width)) return this.width;
    this.width = Math.max(constants.MIN_PANEL_WIDTH, Math.min(width, constants.MAX_PANEL_WIDTH));
    this.ctx.config.values.ui = { ...this.ctx.config.values.ui, panelWidth: this.width };
    this.scheduleSave();
    this.layoutActiveTab();
    return this.width;
  }

  /** Saves the width once the drag settles. */
  scheduleSave() {
    clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => {
      this.saveTimer = null;
      this.ctx.config.save();
    }, constants.PANEL_SAVE_DELAY_MS);
  }

  /** On quit: stop the slide and write any unsaved width now. */
  flush() {
    clearInterval(this.motionTimer);
    if (!this.saveTimer) return;
    clearTimeout(this.saveTimer);
    this.ctx.config.save();
  }
}

module.exports = { PanelLayout };
