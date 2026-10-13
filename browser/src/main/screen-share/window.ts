/** An isolated, browser-owned source picker with exact-frame IPC and safe cancellation. */
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { randomUUID } from 'node:crypto';
import type { BrowserWindow, WebContents, IpcMainEvent, IpcMainInvokeEvent } from 'electron';
import type { AppServices } from '../app/services.ts';
import { SHARE_PICKER, type SharePresentation } from '../../shared/share-picker.ts';
import { PICKER, PICKER_WINDOW, PICKER_WEB } from './constants.ts';

/** Native window ownership and packaged UI location. */
type Deps = Pick<AppServices, 'electron' | 'appDir' | 'windows'>;

/** The picker owns no website cookies and can load only its packaged asset and browser-produced PNGs. */
function pickerSession(deps: Deps, file: string) {
  const session = deps.electron.session.fromPartition(`oya-share-${randomUUID()}`);
  session.setPermissionRequestHandler((_wc, _permission, reply) => reply(false));
  session.setPermissionCheckHandler(() => false);
  session.webRequest.onBeforeRequest((details, reply) =>
    reply({ cancel: details.url !== pathToFileURL(file).href && !details.url.startsWith('data:image/png;') }),
  );
  return session;
}

/** Parent to the requesting page's actual window, not whichever browser happens to be focused. */
function pickerWindow(deps: Deps, page: WebContents, file: string): BrowserWindow {
  const parent = deps.electron.BrowserWindow.fromWebContents(page) ?? deps.windows?.ownerOfContents(page)?.shell.window;
  const webPreferences = pickerPreferences(deps, file);
  return new deps.electron.BrowserWindow({
    ...PICKER_WINDOW,
    parent: parent ?? undefined,
    modal: !!parent,
    webPreferences,
  });
}

/** Load only the packaged isolated bridge on an ephemeral session. */
function pickerPreferences(deps: Deps, file: string) {
  const session = pickerSession(deps, file);
  const preload = path.join(deps.appDir, 'out/preload/share-picker.js');
  return { ...PICKER_WEB, session, preload };
}

/** Native owners can disappear midway through window teardown; stale guards must cancel rather than throw. */
function stillCurrent(current: () => boolean): boolean {
  try {
    return current();
  } catch {
    return false;
  }
}

/** Exactly one result, with renderer failure, Escape, closure and ownership loss all cancelling. */
export class ShareSheet {
  /** Promise resolver stays entirely inside the main process. */
  private readonly pending = Promise.withResolvers<string | null>();
  /** The requesting operation waits for an opaque choice token, never raw capture metadata. */
  readonly result = this.pending.promise;
  /** One native surface for this request. */
  private readonly window: BrowserWindow;
  /** Main-process ownership and document guard checked while the sheet is open. */
  private readonly current: () => boolean;
  /** Bounded startup for a missing or broken picker asset. */
  private readonly loadTimer: NodeJS.Timeout;
  /** Closing on ownership change does not depend on a renderer callback. */
  private readonly ownerTimer: NodeJS.Timeout;
  /** Resolves at most once, before window destruction callbacks run. */
  private finish: ((id: string | null) => void) | null = this.pending.resolve;
  /** A native timer revokes the sheet even when its renderer is busy. */
  private readonly checkOwner = (): void => {
    if (!this.current()) this.answer(null);
  };

  /** Install private IPC before loading any UI. */
  constructor(deps: Deps, page: WebContents, data: SharePresentation, current: () => boolean) {
    this.current = () => stillCurrent(current);
    const file = path.join(deps.appDir, 'out/renderer/share-picker.html');
    this.window = pickerWindow(deps, page, file);
    this.loadTimer = setTimeout(() => this.answer(null), PICKER.LOAD_TIMEOUT_MS);
    this.ownerTimer = setInterval(this.checkOwner, PICKER.OWNER_CHECK_MS);
    this.wire(data);
    this.settings(deps, data);
    void this.window.loadFile(file).catch(() => this.answer(null));
  }

  /** Only the trusted picker's main frame can obtain previews or supply a choice token. */
  private wire(data: SharePresentation): void {
    const wc = this.window.webContents;
    wc.ipc.handle(SHARE_PICKER.READ, (event) => this.read(event, data));
    wc.ipc.on(SHARE_PICKER.ANSWER, (event, id) => this.onAnswer(event, id));
    wc.ipc.on(SHARE_PICKER.READY, (event) => {
      if (event.senderFrame === wc.mainFrame) this.show();
    });
    this.events();
  }

  /** Only a live picker may open the fixed macOS pane; opening settings never grants capture. */
  private settings(deps: Deps, data: SharePresentation): void {
    const wc = this.window.webContents;
    wc.ipc.handle(SHARE_PICKER.SETTINGS, async (event) => {
      this.read(event, data);
      if (!data.canOpenSettings) return false;
      await deps.electron.shell.openExternal(PICKER.SETTINGS_URL);
      this.answer(null);
      return true;
    });
  }

  /** Preview images are available only to this sheet's exact main frame while its request is live. */
  private read(event: IpcMainInvokeEvent, data: SharePresentation): SharePresentation {
    if (!this.finish || !this.current() || event.senderFrame !== this.window.webContents.mainFrame)
      throw Error('Share picker is no longer available');
    return data;
  }

  /** This local UI cannot navigate, open child windows or embed privileged webviews. */
  private events(): void {
    const wc = this.window.webContents;
    wc.setWindowOpenHandler(() => ({ action: 'deny' }));
    wc.on('will-navigate', (event) => event.preventDefault());
    wc.on('will-attach-webview', (event) => event.preventDefault());
    wc.once('render-process-gone', () => this.answer(null));
    this.window.once('closed', () => this.answer(null));
  }

  /** A malformed or foreign IPC message can never select a source. */
  private onAnswer(event: IpcMainEvent, id: unknown): void {
    if (event.senderFrame !== this.window.webContents.mainFrame || (id !== null && typeof id !== 'string')) return;
    this.answer(id);
  }

  /** Show only a fully rendered picker while the requesting page still owns the operation. */
  private show(): void {
    if (!this.finish || !this.current()) return this.answer(null);
    clearTimeout(this.loadTimer);
    this.window.show();
    this.window.focus();
  }

  /** Clear timers and close the UI before resolving, so a next request cannot overlap this sheet. */
  private answer(id: string | null): void {
    const finish = this.finish;
    if (!finish) return;
    this.finish = null;
    clearTimeout(this.loadTimer);
    clearInterval(this.ownerTimer);
    const selected = this.current() ? id : null;
    if (!this.window.isDestroyed()) this.window.destroy();
    finish(selected);
  }
}
