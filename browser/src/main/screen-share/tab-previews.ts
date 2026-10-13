/** Render detached Oya tabs on a temporary native surface without activating or exposing their pages. */
import { randomUUID } from 'node:crypto';
import { setTimeout as pause } from 'node:timers/promises';
import type { BrowserView, BrowserWindow, NativeImage, Rectangle, WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Tab } from '../tabs/types.ts';
import { withinTime } from '../../shared/within-time.ts';
import { PICKER, PICKER_WEB } from './constants.ts';

/** Only browser-owned native surfaces participate in preview rendering. */
type Deps = Pick<AppServices, 'electron'>;

/** Empty native captures have no PNG bytes and must render as placeholders. */
export function previewPNG(image: NativeImage): string {
  return image.isEmpty() ? '' : image.resize(PICKER.THUMBNAIL).toDataURL();
}

/** First paint can lag native attachment; retry briefly without injecting scripts into the website. */
async function painted(contents: WebContents, until: number): Promise<string> {
  while (!contents.isDestroyed() && Date.now() < until) {
    const image = await contents.capturePage(undefined, { stayHidden: true, stayAwake: true }).catch(() => null);
    if (image && !image.isEmpty()) return previewPNG(image);
    await pause(PICKER.PREVIEW_RETRY_MS);
  }
  return '';
}

/** Captures are bounded independently of a stalled renderer's response. */
async function capturePreview(contents: WebContents): Promise<string> {
  return withinTime(painted(contents, Date.now() + PICKER.PREVIEW_TIMEOUT_MS), PICKER.PREVIEW_TIMEOUT_MS).catch(
    () => '',
  );
}

/** A never-shown tab needs a real viewport before its first compositor frame exists. */
function viewport(bounds: Rectangle): Rectangle {
  return { x: 0, y: 0, width: bounds.width || PICKER.PREVIEW_WIDTH, height: bounds.height || PICKER.PREVIEW_HEIGHT };
}

/** A request owns its temporary surface; it never moves an already attached or popup tab. */
export class TabPreviews {
  /** Native window and tab APIs supplied by the browser. */
  private readonly deps: Deps;
  /** Transparent native windows are supported on macOS and Windows only. */
  private readonly staging: boolean;
  /** Non-interactive transparent surface used only during enumeration. */
  private host: BrowserWindow | null = null;
  /** Borrowed views must be detached before their temporary parent is destroyed. */
  private readonly borrowed = new Map<BrowserView, Rectangle>();
  /** Filter the temporary surface out of OS window choices, even after cleanup. */
  sourceId = '';

  /** No surface is created until an eligible detached tab actually needs one. */
  constructor(deps: Deps, platform = process.platform) {
    this.deps = deps;
    this.staging = platform === 'darwin' || platform === 'win32';
  }

  /** Native teardown or an unavailable renderer must not abort other tab previews. */
  capture(tab: Tab): Promise<string> {
    return this.read(tab).catch(() => '');
  }

  /** Mounted tabs capture in place; detached browser views receive a temporary native parent. */
  private async read(tab: Tab): Promise<string> {
    const view = tab.view as BrowserView;
    try {
      if (!tab.window && typeof view.getBounds === 'function') this.attach(view);
      return await capturePreview(view.webContents);
    } finally {
      this.restore(view);
    }
  }

  /** Preserve attached tabs, focus and selection; only detached views are borrowed. */
  private attach(view: BrowserView): void {
    if (!this.staging) return;
    if (this.deps.electron.BrowserWindow.fromWebContents(view.webContents)) return;
    const bounds = view.getBounds();
    const host = this.surface();
    this.borrowed.set(view, bounds);
    view.setBounds(viewport(bounds));
    host.addBrowserView(view);
    host.showInactive();
  }

  /** Opacity and focus policy are set before first display; the surface never accepts user input. */
  private surface(): BrowserWindow {
    if (this.host) return this.host;
    const webPreferences = { ...PICKER_WEB, partition: `oya-preview-${randomUUID()}` };
    this.host = new this.deps.electron.BrowserWindow({ ...PREVIEW_WINDOW, webPreferences });
    this.host.setIgnoreMouseEvents(true);
    this.sourceId = this.host.getMediaSourceId();
    return this.host;
  }

  /** A user may select or move a tab while capture waits; never undo its new owner or layout. */
  private restore(view: BrowserView): void {
    const bounds = this.borrowed.get(view);
    this.borrowed.delete(view);
    if (!bounds || !this.host || this.host.isDestroyed() || !view.webContents || view.webContents.isDestroyed()) return;
    if (this.deps.electron.BrowserWindow.fromWebContents(view.webContents) !== this.host) return;
    this.host.removeBrowserView(view);
    if (bounds.width && bounds.height) view.setBounds(bounds);
  }

  /** No tab renderer may remain attached when the temporary window closes. */
  dispose(): void {
    for (const view of this.borrowed.keys()) this.restore(view);
    if (this.host && !this.host.isDestroyed()) this.host.destroy();
    this.host = null;
  }
}

/** A native compositor surface with no visible pixels, taskbar entry, focus or chrome. */
const PREVIEW_WINDOW = {
  show: false,
  opacity: 0,
  focusable: false,
  skipTaskbar: true,
  frame: false,
  width: PICKER.PREVIEW_WIDTH,
  height: PICKER.PREVIEW_HEIGHT,
};
