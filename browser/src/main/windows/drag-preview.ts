/** A non-interactive native drag card follows the pointer outside the original tab strip. */
import path from 'node:path';
import type { BrowserWindow, BrowserWindowConstructorOptions } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Tab } from '../tabs/types.ts';
import { previewPosition, inTabStrip } from './geometry.ts';
import { DRAG_FRAME_MS, DRAG_PREVIEW_SIZE, DRAG_MAX_MS } from './constants.ts';
/** Sandboxed local pixels, never a page or shell bridge. */
const PREVIEW_OPTIONS: BrowserWindowConstructorOptions = {
  ...DRAG_PREVIEW_SIZE,
  show: false,
  frame: false,
  transparent: true,
  focusable: false,
  skipTaskbar: true,
  alwaysOnTop: true,
  resizable: false,
  webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false, backgroundThrottling: false },
};
/** One gesture owns one disposable, sandboxed preview window. */
export class TabDragPreview {
  /** The card has no preload and never owns a browser tab. */
  private win?: BrowserWindow;
  /** Originating shell scope; another window cannot cancel its gesture. */
  private source?: AppServices;
  /** Native cursor tracking timer. */
  private timer?: ReturnType<typeof setInterval>;
  /** Deadline for a lost pointer release. */
  private deadline = 0;
  /** Only show the card after its local document is ready. */
  private ready = false;
  /** Source closure cancels any pending capture and destroys the card. */
  private readonly closed = () => this.stop();
  /** Creates a local surface without stealing focus or pointer capture. */
  begin(source: AppServices, id: number): void {
    source.shield.requireHumanControl();
    const tab = source.tabs.find(id);
    if (!tab || tab.window) throw new Error('Tab is not in this window');
    const win = this.start(source);
    void this.load(source, win, tab).catch(() => {
      if (this.win === win) this.stop();
    });
  }
  /** Set up tracking before any asynchronous renderer or capture work. */
  private start(source: AppServices): BrowserWindow {
    this.stop();
    this.source = source;
    this.deadline = Date.now() + DRAG_MAX_MS;
    this.win = this.create(source);
    source.shell.window?.once('closed', this.closed);
    this.timer = setInterval(() => this.track(), DRAG_FRAME_MS);
    return this.win;
  }
  /** Local shell pages cannot navigate or spawn a second surface. */
  private create(source: AppServices): BrowserWindow {
    const win = new source.electron.BrowserWindow(PREVIEW_OPTIONS);
    win.setIgnoreMouseEvents(true);
    win.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
    win.webContents.on('will-navigate', (event) => event.preventDefault());
    return win;
  }
  /** First paint shows a useful title immediately; a local thumbnail follows when capture completes. */
  private async load(source: AppServices, win: BrowserWindow, tab: Tab): Promise<void> {
    await win.loadFile(path.join(source.appDir, 'out/renderer/tab-preview/index.html'));
    if (this.win !== win) return;
    await win.webContents.executeJavaScript(
      `document.getElementById('title').textContent = ${JSON.stringify(tab.title || 'Moving tab')}`,
    );
    if (this.win !== win) return;
    this.showReady();
    await this.thumbnail(win, tab).catch(() => {});
  }
  /** A capture failure retains a useful title card instead of removing feedback. */
  private showReady(): void {
    this.ready = true;
    this.track();
  }
  /** Pixels stay in memory and never go to disk, the network or an agent. */
  private async thumbnail(win: BrowserWindow, tab: Tab): Promise<void> {
    const image = await tab.view.webContents.capturePage();
    if (this.win !== win || image.isEmpty()) return;
    const data = image.resize({ width: DRAG_PREVIEW_SIZE.width }).toDataURL();
    await win.webContents.executeJavaScript(
      `{ const image = document.getElementById('thumbnail'); image.src = ${JSON.stringify(data)}; image.style.visibility = 'visible'; }`,
    );
  }
  /** Polling native screen coordinates works across displays without a stream of renderer IPC. */
  private track(): void {
    const source = this.source;
    const win = this.win;
    if (!source || !win) return;
    if (this.expired(source)) return this.stop();
    if (this.ready) this.position(source, win);
  }
  /** Keep the complete card on the pointer's monitor, including negative-coordinate displays. */
  private position(source: AppServices, win: BrowserWindow): void {
    const point = source.electron.screen.getCursorScreenPoint();
    const area = source.electron.screen.getDisplayNearestPoint(point).workArea;
    const position = previewPosition(point, area);
    win.setPosition(position.x, position.y, false);
    if (inTabStrip(point, source.shell.window!.getContentBounds())) win.hide();
    else if (!win.isVisible()) win.showInactive();
  }
  /** Loss of source/control ends feedback even if the renderer missed a release. */
  private expired(source: AppServices): boolean {
    return !source.shell.alive() || !source.control.snapshot().interactive || Date.now() > this.deadline;
  }
  /** Invalidate outstanding asynchronous captures before destroying their destination. */
  private reset(): void {
    this.win = undefined;
    this.source = undefined;
    this.ready = false;
  }
  /** Cancel/reorder removes the card immediately; detach removes it only after the destination is shown. */
  stop(source?: AppServices): void {
    if (source && this.source !== source) return;
    clearInterval(this.timer);
    this.timer = undefined;
    this.source?.shell.window?.removeListener('closed', this.closed);
    const win = this.win;
    this.reset();
    if (win && !win.isDestroyed()) win.destroy();
  }
}
