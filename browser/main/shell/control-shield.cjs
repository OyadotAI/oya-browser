/**
 * Who may touch the page. While an agent has control, a transparent view (the
 * control shield) sits over the active tab and swallows human input, sign-in
 * popups are disabled, and the page menu items go grey. The shield also shows
 * the agent reading the page: a scan and outlines of what it found, drawn over
 * the page rather than into it, so the site never sees them.
 */
const path = require('path');
const {
  TRANSPARENT,
  HUMAN_MENU_ITEMS,
  MAX_ANALYSIS_BOXES,
  SHIELD_TRACK_MS,
  SHIELD_TRACK_FOR_MS,
} = require('./constants.cjs');
const { drivenElsewhere } = require('../../control-state.cjs');

/**
 * Measures elements in the isolated world; it only reads. Each is found as the
 * analyzer finds it (shadow roots and iframes included), placed through any
 * same-origin frames, and mapped into the visual viewport so a pinch-zoom still
 * lines up. `__WANTED__` becomes the `[id, type, selector]` list.
 */
const MEASURE_JS = `(() => {
  const vv = window.visualViewport || { offsetLeft: 0, offsetTop: 0, scale: 1 };
  const find = (id, selector) =>
    window.__acFindElement?.(id) || window.__acQueryShadow?.(selector) || document.querySelector(selector);
  const place = (el) => {
    const r = el.getBoundingClientRect();
    let x = r.left, y = r.top;
    for (let f = el.ownerDocument.defaultView?.frameElement; f; f = f.ownerDocument.defaultView?.frameElement) {
      const fr = f.getBoundingClientRect();
      x += fr.left + f.clientLeft;
      y += fr.top + f.clientTop;
    }
    const k = vv.scale;
    return { x: (x - vv.offsetLeft) * k, y: (y - vv.offsetTop) * k, w: r.width * k, h: r.height * k };
  };
  // Painted, and on top somewhere: not faded out, hidden, covered by something else, or clipped away.
  const shows = (el) => {
    if (el.checkVisibility && !el.checkVisibility({ opacityProperty: true, visibilityProperty: true })) return false;
    const r = el.getBoundingClientRect();
    const root = el.getRootNode();
    const lands = ([fx, fy]) => {
      const hit = root.elementFromPoint?.(r.left + r.width * fx, r.top + r.height * fy);
      return !!hit && (hit === el || el.contains(hit));
    };
    return [[0.5, 0.5], [0.2, 0.2], [0.8, 0.2], [0.2, 0.8], [0.8, 0.8]].some(lands);
  };
  return (__WANTED__).map(([id, type, selector]) => {
    const el = find(id, selector);
    if (!el || (__STRICT__ && !shows(el))) return null;
    const box = place(el);
    return box.w && box.h ? { id, type, ...box } : null;
  }).filter(Boolean);
})()`;

/**
 * Measures the visible elements an analysis found. `strict` also leaves out any the
 * page does not actually show (faded, covered or clipped): worth it once per analysis,
 * not on every re-measure while the outlines follow the page.
 */
function analysisBoxesJs(elements, strict = false) {
  const wanted = elements.filter((e) => e.visible).slice(0, MAX_ANALYSIS_BOXES);
  const list = JSON.stringify(wanted.map((e) => [e.id, e.type, e.selector]));
  return MEASURE_JS.replace('__WANTED__', () => list).replace('__STRICT__', String(strict));
}

/** Whether an agent holds the page: anything that re-analyzes it would renumber the agent's element ids. */
const agentDriving = (ctx) => !ctx.control.snapshot().interactive;

/** What a read of the page answers while an agent holds it. */
const AGENT_HOLDS_PAGE = 'The agent is using this page. Take control to read it.';

/** The shield view and the popups it disables. */
class ControlShield {
  /** `ctx` is the main-process context (see main.js). */
  constructor(ctx) {
    /** The main-process context. */
    this.ctx = ctx;
    /** The shield's BrowserView, created on first use. */
    this.view = null;
    /** Sign-in popups, enabled only while a person has control. */
    this.popups = new Set();
    /** The outlines being followed right now (`{ timer }`), or null; a newer run replaces it. */
    this.tracking = null;
  }

  /** Throws unless a person holds control; every page-touching IPC call checks this. */
  requireHumanControl() {
    if (!this.ctx.control.snapshot().interactive) throw new Error('Take control before interacting with this page');
  }

  /** A tab that takes focus while an agent drives hands it back to the shell. */
  keepFocusOnShell() {
    if (!this.ctx.control.snapshot().interactive) this.ctx.shell.window?.webContents.focus();
  }

  /** The control state changed: tell the shell, and fence the page accordingly. */
  controlChanged(state) {
    this.ctx.shell.send('control-state', state);
    this.ctx.recorder.controlLost(state);
    this.enableMenus(state.interactive);
    for (const popup of this.popups) if (!popup.isDestroyed()) popup.setEnabled(!drivenElsewhere(state));
    if (this.ctx.shell.alive()) this.sync();
  }

  /** Greys out the page commands in the application menu. */
  enableMenus(interactive) {
    for (const id of HUMAN_MENU_ITEMS) {
      const item = this.ctx.electron.Menu.getApplicationMenu()?.getMenuItemById(id);
      if (item) item.enabled = interactive;
    }
  }

  /** A new sign-in popup: disabled only while someone else drives, forgotten once closed. */
  adoptPopup(childWindow) {
    this.popups.add(childWindow);
    childWindow.setEnabled(!drivenElsewhere(this.ctx.control.snapshot()));
    childWindow.once('closed', () => this.popups.delete(childWindow));
  }

  /** Creates the shield view, once. */
  prepare() {
    if (this.view) return;
    const webPreferences = { sandbox: true, contextIsolation: true, nodeIntegration: false };
    this.view = new this.ctx.electron.BrowserView({ webPreferences });
    this.view.setBackgroundColor(TRANSPARENT);
    this.view.webContents.loadFile(path.join(__dirname, '..', '..', 'renderer/control-shield.html'));
    this.ctx.shortcuts.install(this.view.webContents);
  }

  /** Whether the page should be covered right now. */
  shouldCover() {
    const { shell, overlays, control } = this.ctx;
    return shell.browsingMode && !overlays.names.size && !control.snapshot().interactive;
  }

  /** Puts the shield over the active tab, or takes it away. */
  sync() {
    const win = this.ctx.shell.window;
    if (!win || win.isDestroyed()) return;
    if (!this.shouldCover()) return this.uncover(win);
    this.prepare();
    const view = this.ctx.tabs.getActiveView();
    if (view) this.cover(win, view);
  }

  /** An agent began reading the page: the shield starts its scan, and stops following the last outlines. */
  analysisStarted(view) {
    this.stopTracking();
    this.tell(view, { phase: 'scan' });
  }

  /** The analysis is back: outline what it found that the page really shows, then keep the outlines on their elements. A failed measurement outlines nothing. */
  async analysisFinished(view, raw) {
    this.stopTracking();
    if (!this.showing(view)) return;
    const elements = raw?.data?.elements || [];
    const boxes = (await this.measure(view, analysisBoxesJs(elements, true))) || [];
    this.tell(view, { phase: 'found', boxes });
    const shown = new Set(boxes.map((b) => b.id));
    this.track(view, analysisBoxesJs(elements.filter((e) => shown.has(e.id))), Date.now() + SHIELD_TRACK_FOR_MS);
  }

  /** Where the elements sit now, in the shield's pixels (the page may be zoomed, the shield is not); null when the page cannot be read. */
  async measure(view, js, options) {
    const boxes = await this.ctx.world.worldEval(view, js, options).catch(() => null);
    if (!boxes) return null;
    const k = view.webContents.getZoomFactor() / this.view.webContents.getZoomFactor();
    return boxes.map((b) => ({ ...b, x: b.x * k, y: b.y * k, w: b.w * k, h: b.h * k }));
  }

  /** Re-measures every SHIELD_TRACK_MS until `until`, so the outlines glide with the page as it scrolls or reflows. */
  track(view, js, until) {
    const run = (this.tracking = {});
    run.timer = setTimeout(() => this.follow(run, view, js, until), SHIELD_TRACK_MS);
  }

  /** One re-measure of a tracking run; it tells the page, then schedules the next, unless the run was stopped or is over. A page that cannot be read has navigated away: its outlines fade and following stops. */
  async follow(run, view, js, until) {
    if (this.tracking !== run || !this.showing(view) || Date.now() > until) return;
    const boxes = await this.measure(view, js, { retry: false });
    if (this.tracking !== run) return;
    // The page could not be read: it has navigated, so these outlines belong to a page that is gone.
    if (!boxes) return this.tell(view, { phase: 'move', boxes: [] });
    this.tell(view, { phase: 'move', boxes });
    run.timer = setTimeout(() => this.follow(run, view, js, until), SHIELD_TRACK_MS);
  }

  /** Stops following the outlines. */
  stopTracking() {
    clearTimeout(this.tracking?.timer);
    this.tracking = null;
  }

  /** Whether the shield is over `view` right now. */
  showing(view) {
    return !!this.view && this.shouldCover() && this.ctx.tabs.getActiveView() === view;
  }

  /** Hands the shield page one update, when it is over `view`. */
  tell(view, update) {
    if (!this.showing(view)) return;
    this.view.webContents.executeJavaScript(`window.oyaShield?.(${JSON.stringify(update)})`).catch(() => {});
  }

  /** Takes the shield off the window. */
  uncover(win) {
    this.stopTracking();
    if (this.view) win.removeBrowserView(this.view);
  }

  /** Lays the shield exactly over `view`, on top, and keeps keyboard focus off the page. */
  cover(win, view) {
    this.view.setBounds(view.getBounds());
    if (!win.getBrowserViews().includes(this.view)) win.addBrowserView(this.view);
    win.setTopBrowserView(this.view);
    if (view.webContents.isFocused()) win.webContents.focus();
  }
}

module.exports = { ControlShield, agentDriving, AGENT_HOLDS_PAGE };
