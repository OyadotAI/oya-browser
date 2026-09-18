/**
 * The shell window: the local page that holds the toolbar, tab strip and dev
 * panel, with the tabs' views laid over it. Everything the main process tells
 * the shell goes through send().
 */
const path = require('path');
const { redact } = require('../../scripts/diagnostics.cjs');
const { JSON_INDENT } = require('../app/constants.cjs');
const {
  WINDOW_SIZE,
  TRAFFIC_LIGHTS,
  SHELL_BACKGROUND,
  DEFAULT_PANEL_WIDTH,
  DEV_LOG_MAX_CHARS,
} = require('./constants.cjs');

/** The browser folder, where the shell's page, preload and icons live. */
const APP_DIR = path.join(__dirname, '..', '..');

/** The shell page's preload bridge, isolated from the page's own scripts. */
const SHELL_WEB_PREFERENCES = {
  preload: path.join(APP_DIR, 'preload.js'),
  contextIsolation: true,
  nodeIntegration: false,
};

/** macOS gets an inset title bar with the traffic lights drawn over the toolbar. */
function titleBar(mac) {
  return { titleBarStyle: mac ? 'hiddenInset' : 'default', trafficLightPosition: mac ? TRAFFIC_LIGHTS : undefined };
}

/** The window and whether it is showing web pages yet. */
class ShellWindow {
  /** `ctx` is the main-process context (see main.js). */
  constructor(ctx) {
    /** The main-process context. */
    this.ctx = ctx;
    /** The BrowserWindow, once created. */
    this.window = null;
    /** False on the setup screen; true once pages are shown. */
    this.browsingMode = false;
  }

  /** Whether the window exists and has not been destroyed. */
  alive() {
    return !!this.window && !this.window.isDestroyed();
  }

  /** Sends one message to the shell page, if it is still there. */
  send(channel, data) {
    if (this.alive()) this.window.webContents.send(channel, data);
  }

  /** Mirrors a control-socket message into the dev panel's log, secrets redacted. */
  devLog(direction, type, data) {
    if (!this.alive()) return;
    const entry = { ts: Date.now(), dir: direction, type, data: JSON.stringify(redact(data), null, JSON_INDENT) };
    if (entry.data && entry.data.length > DEV_LOG_MAX_CHARS) {
      entry.data = entry.data.slice(0, DEV_LOG_MAX_CHARS) + '\n... (truncated)';
    }
    this.window.webContents.send('dev-log', entry);
  }

  /** Shell appearance is independent of the websites' preferred color scheme. */
  background() {
    const ui = this.ctx.config.values.ui;
    const dark = ui?.theme === 'dark' || (ui?.theme !== 'light' && this.ctx.electron.nativeTheme.shouldUseDarkColors);
    return dark ? SHELL_BACKGROUND.dark : SHELL_BACKGROUND.light;
  }

  /** Opens the window and wires what it listens to. */
  create() {
    this.ctx.layout.width = Number(this.ctx.config.values.ui?.panelWidth) || DEFAULT_PANEL_WIDTH;
    this.window = new this.ctx.electron.BrowserWindow(this.options());
    this.window.loadFile(path.join(APP_DIR, 'renderer/index.html')).then(() => this.ctx.shield.prepare());
    this.ctx.shortcuts.install(this.window.webContents);
    this.lockToShellPage();
    this.followTheme();
    if (process.env.OYA_DOCKER === 'true') this.window.maximize();
    this.window.on('resize', () => this.ctx.layout.layoutActiveTab());
  }

  /** The window's construction options. */
  options() {
    const mac = process.platform === 'darwin';
    return {
      ...WINDOW_SIZE,
      icon: path.join(APP_DIR, 'build', mac ? 'icon.icns' : 'icon_1024.png'),
      ...titleBar(mac),
      backgroundColor: this.background(),
      webPreferences: SHELL_WEB_PREFERENCES,
    };
  }

  /** The shell is a local page: it never navigates away or opens windows of its own. */
  lockToShellPage() {
    const contents = this.window.webContents;
    contents.on('will-navigate', (event) => event.preventDefault());
    contents.setWindowOpenHandler(() => ({ action: 'deny' }));
    contents.on('did-finish-load', () => this.ctx.layout.layoutActiveTab());
  }

  /** Repaints the window and tells the page when the system theme changes. */
  followTheme() {
    const { nativeTheme } = this.ctx.electron;
    nativeTheme.on('updated', () => {
      if (!this.alive()) return;
      this.window.setBackgroundColor(this.background());
      this.send('shell-appearance', nativeTheme.shouldUseDarkColors);
    });
  }
}

module.exports = { ShellWindow };
