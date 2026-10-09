/** Isolated browser-owned sheets for human prompts and confirmations on macOS and Windows. */
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { randomUUID } from 'node:crypto';
import type { BrowserWindow, WebContents, IpcMainEvent } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { NativeDialogInfo, NativeDialogReply } from '../native/index.ts';
import { NATIVE_DIALOG } from '../../shared/native-dialog.ts';
import { dialogPresentation } from './presentation.ts';
import { DIALOG_WINDOW_OPTIONS, DIALOG_WEB_PREFERENCES } from './constants.ts';
/** The owning native window is resolved at presentation, including after tab transfer. */
type Deps = Pick<AppServices, 'electron' | 'appDir' | 'windows'>;
/** Every sheet gets a private ephemeral session with no website cookies, network or permissions. */
function makeWindow(deps: Deps, page: WebContents, file: string): BrowserWindow {
  const parent = deps.electron.BrowserWindow.fromWebContents(page) ?? deps.windows?.ownerOfContents(page)?.shell.window;
  const webPreferences = sheetPreferences(deps, file);
  return new deps.electron.BrowserWindow({
    ...DIALOG_WINDOW_OPTIONS,
    parent: parent ?? undefined,
    modal: !!parent,
    webPreferences,
  });
}
/** No site cookies, network, filesystem navigation or permission grants enter the sheet. */
function privateSession(deps: Deps, file: string) {
  const session = deps.electron.session.fromPartition(`oya-dialog-${randomUUID()}`);
  session.setPermissionRequestHandler((_wc, _permission, reply) => reply(false));
  session.setPermissionCheckHandler(() => false);
  session.webRequest.onBeforeRequest((details, reply) => reply({ cancel: details.url !== pathToFileURL(file).href }));
  return session;
}
/** Own one sheet's lifecycle; closing on transfer is not a native dialog answer. */
class Sheet {
  /** Cancel callbacks become no-ops before any UI is destroyed. */
  private live = true;
  /** The dedicated native sheet. */
  private readonly window: BrowserWindow;
  /** Never keep a renderer blocked indefinitely if browser UI cannot load. */
  private readonly timer: NodeJS.Timeout;
  /** Native callback retained only in the browser process. */
  private readonly reply: NativeDialogReply;
  /** Wire exact-sender IPC before loading the trusted static asset. */
  constructor(deps: Deps, page: WebContents, info: NativeDialogInfo, reply: NativeDialogReply) {
    const file = path.join(deps.appDir, 'out/renderer/native-dialog.html');
    this.window = makeWindow(deps, page, file);
    this.reply = reply;
    this.timer = setTimeout(() => this.answer(false), NATIVE_DIALOG.LOAD_TIMEOUT_MS);
    this.wire(info);
    this.window.loadFile(file).catch(() => this.answer(false));
  }
  /** Closing a sheet on ownership transfer or page cancellation never decides the page's dialog. */
  close = (): void => {
    this.live = false;
    clearTimeout(this.timer);
    if (!this.window.isDestroyed()) this.window.destroy();
  };
  /** Only the sheet's own main frame can read or answer its specific pending dialog. */
  private wire(info: NativeDialogInfo): void {
    const wc = this.window.webContents;
    this.events();
    wc.ipc.handle(NATIVE_DIALOG.READ, (event) => {
      if (!this.live || event.senderFrame !== wc.mainFrame) throw Error('Dialog is no longer available');
      return dialogPresentation(info);
    });
    wc.ipc.on(NATIVE_DIALOG.ANSWER, (event, accept, text) => this.onAnswer(event, accept, text));
  }
  /** Destruction cancels safely; only the fully populated, trusted sheet can request first display. */
  private events(): void {
    const wc = this.window.webContents;
    this.navigation();
    wc.ipc.on(NATIVE_DIALOG.READY, (event) => {
      if (event.senderFrame === wc.mainFrame) this.show();
    });
    this.window.once('closed', () => this.answer(false));
    wc.once('render-process-gone', () => this.answer(false));
  }
  /** No untrusted navigation or child window may inherit the private dialog bridge. */
  private navigation(): void {
    const wc = this.window.webContents;
    wc.setWindowOpenHandler(() => ({ action: 'deny' }));
    wc.on('will-navigate', (event) => event.preventDefault());
    wc.on('will-attach-webview', (event) => event.preventDefault());
  }
  /** Invalid messages cannot decide a native dialog or coerce an arbitrary object into prompt text. */
  private onAnswer(event: IpcMainEvent, accept: unknown, text: unknown): void {
    if (!this.live || event.senderFrame !== this.window.webContents.mainFrame) return;
    if (typeof accept !== 'boolean' || typeof text !== 'string') return;
    this.answer(accept, text);
  }
  /** Paint the complete sheet once, then focus its keyboard-first safe default. */
  private show(): void {
    if (!this.live) return;
    clearTimeout(this.timer);
    this.window.show();
    this.window.focus();
  }
  /** Close before calling back: the page may synchronously open another dialog. */
  private answer(accept: boolean, text = ''): void {
    if (!this.live) return;
    this.close();
    this.reply(accept, text);
  }
}
/** Composition root exposes presentation without leaking the Electron module into decision policy. */
export function dialogPresenter(deps: Deps) {
  return (page: WebContents, info: NativeDialogInfo, reply: NativeDialogReply): (() => void) =>
    new Sheet(deps, page, info, reply).close;
}

/** Keep the bridge path inside the packaged application, never in a requesting site's configuration. */
function sheetPreferences(deps: Deps, file: string) {
  const session = privateSession(deps, file);
  const preload = path.join(deps.appDir, 'out/preload/dialog.js');
  return { ...DIALOG_WEB_PREFERENCES, session, preload };
}
