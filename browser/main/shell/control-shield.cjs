/**
 * Who may touch the page. While an agent has control, a transparent view (the
 * control shield) sits over the active tab and swallows human input, sign-in
 * popups are disabled, and the page menu items go grey. The shield also shows
 * the agent reading the page: a scan and outlines of what it found, drawn over
 * the page rather than into it, so the site never sees them.
 */
const path = require('path');
const { TRANSPARENT, HUMAN_MENU_ITEMS, MAX_ANALYSIS_BOXES } = require('./constants.cjs');
const { drivenElsewhere } = require('../../control-state.cjs');

/** Measures the visible elements an analysis found, by their selectors, in the isolated world; it only reads. */
function analysisBoxesJs(elements) {
  const wanted = elements.filter((e) => e.visible).slice(0, MAX_ANALYSIS_BOXES);
  return `(${JSON.stringify(wanted.map((e) => [e.id, e.type, e.selector]))}).map(([id, type, selector]) => {
    const r = document.querySelector(selector)?.getBoundingClientRect();
    return r && r.width && r.height ? { id, type, x: r.left, y: r.top, w: r.width, h: r.height } : null;
  }).filter(Boolean)`;
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

  /** An agent began reading the page: the shield starts its scan. */
  analysisStarted(view) {
    this.tell(view, { phase: 'scan' });
  }

  /** The analysis is back: outline what it found. A failed measurement outlines nothing. */
  async analysisFinished(view, raw) {
    if (!this.showing(view)) return;
    const js = analysisBoxesJs(raw?.data?.elements || []);
    const boxes = await this.ctx.world.worldEval(view, js).catch(() => []);
    this.tell(view, { phase: 'found', boxes: boxes || [] });
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
