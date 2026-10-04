/**
 * The shell window: the local page that holds the toolbar, tab strip and dev
 * panel, with the tabs' views laid over it. Everything the main process tells
 * the shell goes through send().
 */
import path from 'node:path';
import type { BrowserWindow, BrowserWindowConstructorOptions, LoadFileOptions, WebPreferences } from 'electron';
import type { AppServices } from '../app/services.ts';
import { redact } from '../../workflow/index.ts';
import { JSON_INDENT } from '../app/constants.ts';
import { WINDOW_SIZE, TRAFFIC_LIGHTS, SHELL_BACKGROUND, PANEL_WIDTH, DEV_LOG_MAX_CHARS } from './constants.ts';
import { holdStill, inContainer } from './hold-still.ts';

/** The services the shell window uses. */
type Deps = Pick<AppServices, 'electron' | 'appDir' | 'config' | 'layout' | 'shortcuts' | 'shield' | 'tabs'>;

/** The shell page's preload bridge (built to out/preload/), isolated from the page's own scripts. */
function shellWebPreferences(appDir: string): WebPreferences {
  return { preload: path.join(appDir, 'out', 'preload', 'index.js'), contextIsolation: true, nodeIntegration: false };
}

/** macOS gets an inset title bar with the traffic lights drawn over the toolbar. */
function titleBar(mac: boolean): BrowserWindowConstructorOptions {
  return { titleBarStyle: mac ? 'hiddenInset' : 'default', trafficLightPosition: mac ? TRAFFIC_LIGHTS : undefined };
}

/** A shell whose window is open. */
interface OpenShell {
  /** The open window. */
  window: BrowserWindow;
}

/** The window and whether it is showing web pages yet. */
export class ShellWindow {
  /** The BrowserWindow, once created. */
  window: BrowserWindow | null = null;
  /** False on the setup screen; true once pages are shown. */
  browsingMode = false;
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` is the main-process context (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Whether the window exists and has not been destroyed. */
  alive(): this is OpenShell {
    return !!this.window && !this.window.isDestroyed();
  }

  /** Sends one message to the shell page, if it is still there. */
  send(channel: string, data?: unknown): void {
    if (this.alive()) this.window.webContents.send(channel, data);
  }

  /** Mirrors a control-socket message into the dev panel's log, secrets redacted. */
  devLog(direction: string, type: string, data: unknown): void {
    if (!this.alive()) return;
    const entry = { ts: Date.now(), dir: direction, type, data: JSON.stringify(redact(data), null, JSON_INDENT) };
    if (entry.data && entry.data.length > DEV_LOG_MAX_CHARS) {
      entry.data = entry.data.slice(0, DEV_LOG_MAX_CHARS) + '\n... (truncated)';
    }
    this.window.webContents.send('dev-log', entry);
  }

  /** Shell appearance is independent of the websites' preferred color scheme. */
  background(): string {
    return this.dark() ? SHELL_BACKGROUND.dark : SHELL_BACKGROUND.light;
  }

  /** Loads the shell page in its theme, then readies the control shield. */
  private loadShellPage(win: BrowserWindow): void {
    void holdStill(win, inContainer()); // before the page loads, so its loops never start
    const page = path.join(this.deps.appDir, 'out', 'renderer', 'index.html');
    win.loadFile(page, this.firstPaint()).then(() => this.deps.shield.prepare());
  }

  /** The shell page's address options: its theme rides in the query so the very first paint is already in it (src/renderer/public/first-paint.js). */
  private firstPaint(): LoadFileOptions {
    return { query: { theme: this.dark() ? 'dark' : 'light' } };
  }

  /** Whether the shell is dark: chosen so, or following a dark system. */
  private dark(): boolean {
    const ui = this.deps.config.values.ui;
    return ui?.theme === 'dark' || (ui?.theme !== 'light' && this.deps.electron.nativeTheme.shouldUseDarkColors);
  }

  /** Opens the window and wires what it listens to. */
  create(): void {
    this.deps.layout.width = Number(this.deps.config.values.ui?.panelWidth) || PANEL_WIDTH;
    const win = (this.window = new this.deps.electron.BrowserWindow(this.options()));
    this.loadShellPage(win);
    this.deps.shortcuts.install(win.webContents);
    this.lockToShellPage(win);
    this.followTheme();
    this.fitContainer(win);
    win.on('resize', () => this.deps.layout.layoutActiveTab());
  }

  /** In a container the window fills the virtual screen. */
  private fitContainer(win: BrowserWindow): void {
    if (inContainer()) win.maximize();
  }

  /** The window's construction options. */
  private options(): BrowserWindowConstructorOptions {
    const mac = process.platform === 'darwin';
    return {
      ...WINDOW_SIZE,
      icon: path.join(this.deps.appDir, 'build', mac ? 'icon.icns' : 'icon_1024.png'),
      ...titleBar(mac),
      backgroundColor: this.background(),
      webPreferences: shellWebPreferences(this.deps.appDir),
    };
  }

  /**
   * The shell is a local page: it never navigates away or opens windows of its
   * own, and it is never zoomed. Its layout is in window pixels; Chromium
   * remembers a zoom per origin, so one left by an older build is undone here.
   */
  private lockToShellPage(win: BrowserWindow): void {
    const contents = win.webContents;
    contents.on('will-navigate', (event) => event.preventDefault());
    contents.setWindowOpenHandler(() => ({ action: 'deny' }));
    contents.on('did-finish-load', () => {
      contents.setZoomLevel(0);
      this.deps.layout.layoutActiveTab();
      // A fast connection opens tabs while the page is still loading its scripts, and a late one misses that list.
      this.deps.tabs.sendTabList();
    });
  }

  /** Repaints the window and tells the page when the system theme changes. */
  private followTheme(): void {
    const { nativeTheme } = this.deps.electron;
    nativeTheme.on('updated', () => {
      if (!this.alive()) return;
      this.window.setBackgroundColor(this.background());
      this.send('shell-appearance', nativeTheme.shouldUseDarkColors);
    });
  }
}
