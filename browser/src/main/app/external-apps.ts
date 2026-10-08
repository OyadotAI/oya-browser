/** Explicit, human-approved Zoom handoff; never arbitrary OS protocol execution. */
import type { WebContents, MessageBoxOptions } from 'electron';
import type { AppServices } from './services.ts';
import { EXTERNAL_APP_URL_LIMIT, OPEN_EXTERNAL_APP_CHOICE } from './constants.ts';
/** Control, active surface, policy and native APIs remain injectable. */
type Deps = Pick<AppServices, 'electron' | 'shell' | 'tabs' | 'control' | 'governance'>;
/** Recognize a Zoom app attempt even when malformed so it cannot become a broken tab or search. */
const isZoomScheme = (raw: string): boolean => /^zoommtg:/i.test(raw.trim());
/** Accept only Zoom meeting joins; other protocols and Zoom app commands remain blocked. */
export function zoomMeetingLink(raw: string): string | null {
  if (raw.length > EXTERNAL_APP_URL_LIMIT || /[\p{Cc}\s]/u.test(raw)) return null;
  const url = URL.parse(raw);
  if (!url || url.protocol !== 'zoommtg:' || url.username || url.password || url.port || url.hash) return null;
  if (url.hostname !== 'zoom.us' && !url.hostname.endsWith('.zoom.us')) return null;
  if (url.pathname !== '/join') return null;
  return url.href;
}
/** One pending native confirmation across all tabs prevents popup storms. */
export class ExternalApps {
  /** Main-process collaborators. */
  private readonly deps: Deps;
  /** A prompt or OS launch is already in progress. */
  private pending = false;
  /** Navigation listeners are installed at most once per surface. */
  private readonly wired = new WeakSet<WebContents>();
  /** Constructed once, before tabs begin loading. */
  constructor(deps: Deps) {
    this.deps = deps;
  }
  /** Include subframes, redirects and popup tabs without injecting page scripts. */
  wire(contents: WebContents): void {
    if (this.wired.has(contents)) return;
    this.wired.add(contents);
    contents.on('will-frame-navigate', (event) => {
      if (this.request(event.url, contents)) event.preventDefault();
    });
    contents.on('will-redirect', (event, url) => {
      if (this.request(event.url || url, contents)) event.preventDefault();
    });
  }
  /** Synchronously claim the navigation, then ask asynchronously without changing the page. */
  request(raw: string, contents?: WebContents): boolean {
    if (!isZoomScheme(raw)) return false;
    const url = zoomMeetingLink(raw.trim());
    if (!url || this.pending || !contents || !this.allowed(contents)) return true;
    this.begin(url, contents);
    return true;
  }
  /** Keep the prompt locked until every success or failure path settles. */
  private begin(url: string, contents: WebContents): void {
    this.pending = true;
    void this.launch(url, contents, contents.getURL())
      .catch(() => this.failed())
      .catch(() => {})
      .finally(() => {
        this.pending = false;
      });
  }
  /** Managed runtimes and agent control cannot launch a local desktop application. */
  private allowed(contents: WebContents): boolean {
    if (contents.isDestroyed() || this.deps.governance.configuration || !this.deps.control.snapshot().interactive)
      return false;
    return contents === this.deps.tabs.getActiveView()?.webContents || contents === this.deps.shell.window?.webContents;
  }
  /** Recheck ownership and source after the person answers, before leaving the sandbox. */
  private async launch(url: string, contents: WebContents, source: string): Promise<void> {
    const result = await this.ask(this.confirmation(source));
    if (result.response !== OPEN_EXTERNAL_APP_CHOICE || !this.allowed(contents) || contents.getURL() !== source) return;
    await this.deps.electron.shell.openExternal(url);
  }
  /** Cancellation is the default; only a person's affirmative click can launch. */
  private confirmation(source: string): MessageBoxOptions {
    return {
      type: 'question',
      message: 'Open Zoom?',
      detail: `${this.sourceLabel(source)} wants to open the Zoom app. Only continue if you trust this meeting link.`,
      buttons: ['Cancel', 'Open Zoom'],
      defaultId: 0,
      cancelId: 0,
    };
  }
  /** Never display meeting passwords, private paths, or query strings in prompts. */
  private sourceLabel(raw: string): string {
    const url = URL.parse(raw);
    return url && ['http:', 'https:'].includes(url.protocol) ? url.origin : 'Oya Browser';
  }
  /** Missing app handlers leave the original meeting page available for web joining. */
  private async failed(): Promise<void> {
    await this.ask({
      type: 'info',
      message: 'Could not open Zoom',
      detail:
        'Install or open the Zoom desktop app, then try again. You can also use the meeting page’s browser-join option.',
      buttons: ['OK'],
    });
  }
  /** Electron requires an existing window or the unparented overload, never null. */
  private ask(options: MessageBoxOptions) {
    const window = this.deps.shell.window;
    return window && !window.isDestroyed()
      ? this.deps.electron.dialog.showMessageBox(window, options)
      : this.deps.electron.dialog.showMessageBox(options);
  }
}
