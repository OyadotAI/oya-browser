/**
 * Overlays the shell raises over the page (menus, dialogs). A BrowserView
 * always paints above the shell's own HTML, so the page view is taken away
 * while an overlay is up, and a screenshot of it stands in as the backdrop.
 */
const crypto = require('crypto');
const { OVERLAYS, BACKDROP_WAIT_MS } = require('./constants.cjs');

/** The overlays up right now and the backdrops being painted. */
class Overlays {
  /** `ctx` is the main-process context (see main.js). */
  constructor(ctx) {
    /** The main-process context. */
    this.ctx = ctx;
    /** Names of the overlays currently shown. */
    this.names = new Set();
    /** Backdrop token → resolver, while the shell paints it. */
    this.backdropWaiters = new Map();
  }

  /** The active tab, when pages are being shown at all. */
  shownView() {
    const view = this.ctx.tabs.getActiveView();
    return view && this.ctx.shell.browsingMode ? view : null;
  }

  /** Raises an overlay: freeze the page as a backdrop, then take the view away. */
  async show(name = 'legacy') {
    if (!OVERLAYS.includes(name)) return;
    const view = this.ctx.tabs.getActiveView();
    if (!this.names.size && view && this.ctx.shell.browsingMode) await this.freeze(view);
    this.names.add(name);
    if (view && this.ctx.shell.browsingMode) this.ctx.shell.window.removeBrowserView(view);
    this.ctx.shield.sync();
  }

  /** Hands the shell a screenshot of the page and waits (briefly) for it to be painted. */
  async freeze(view) {
    try {
      const screenshot = await view.webContents.capturePage();
      const token = crypto.randomUUID();
      await this.paintBackdrop(token, { token, image: screenshot.toDataURL(), bounds: view.getBounds() });
    } catch {
      /* A crashed page has no frame to preserve; the reload status remains visible. */
    }
  }

  /** Sends the backdrop and resolves when the shell says it is up, or after a short wait. */
  paintBackdrop(token, backdrop) {
    return new Promise((resolve) => {
      const timer = setTimeout(() => this.settle(token, resolve), BACKDROP_WAIT_MS);
      this.backdropWaiters.set(token, () => this.settle(token, resolve, timer));
      this.ctx.shell.send('page-backdrop', backdrop);
    });
  }

  /** Stops waiting for one backdrop. */
  settle(token, resolve, timer) {
    clearTimeout(timer);
    this.backdropWaiters.delete(token);
    resolve();
  }

  /** The shell painted a backdrop. */
  backdropReady(token) {
    this.backdropWaiters.get(token)?.();
  }

  /** Lowers an overlay; with none left, the page comes back. */
  hide(name = 'legacy') {
    this.names.delete(name);
    const view = this.shownView();
    if (view && !this.names.size) {
      this.ctx.shell.window.setBrowserView(view);
      this.ctx.layout.layoutActiveTab();
    }
    if (!this.names.size) this.ctx.shell.send('page-backdrop', null);
  }
}

module.exports = { Overlays };
