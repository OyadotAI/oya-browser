/**
 * Who may touch the page. While an agent has control, a transparent view (the
 * control shield) sits over the active tab and swallows human input, sign-in
 * popups are disabled, and the page menu items go grey.
 */
const path = require('path');
const { TRANSPARENT, HUMAN_MENU_ITEMS } = require('./constants.cjs');

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
    for (const popup of this.popups) if (!popup.isDestroyed()) popup.setEnabled(state.interactive);
    if (this.ctx.shell.alive()) this.sync();
  }

  /** Greys out the page commands in the application menu. */
  enableMenus(interactive) {
    for (const id of HUMAN_MENU_ITEMS) {
      const item = this.ctx.electron.Menu.getApplicationMenu()?.getMenuItemById(id);
      if (item) item.enabled = interactive;
    }
  }

  /** A new sign-in popup: disabled unless a person has control, forgotten once closed. */
  adoptPopup(childWindow) {
    this.popups.add(childWindow);
    childWindow.setEnabled(this.ctx.control.snapshot().interactive);
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

module.exports = { ControlShield };
