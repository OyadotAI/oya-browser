/**
 * Windows a tab's page opens, and its right-click menu. An anonymous
 * target="_blank" becomes a tab; a sign-in popup and a window the page named
 * stay real windows, protected and adopted onto the tab list.
 */
import type { BrowserWindow, HandlerDetails, LoadURLOptions, WindowOpenHandlerResponse } from 'electron';
import type { AppServices } from '../app/services.ts';
import { isAuthPopup, opensNamedWindow } from './auth-popup.ts';
import { GmailPopup } from './gmail-popup.ts';
import { ContextMenu } from './context-menu.ts';
import { AUTH_POPUP_SIZE, LOCAL_FILE } from './constants.ts';
import type { Tab, TabView } from './types.ts';

/** The services a tab's windows and menu use. */
export type TabWindowsDeps = Pick<
  AppServices,
  | 'externalApps'
  | 'tabs'
  | 'persona'
  | 'protection'
  | 'recorder'
  | 'electron'
  | 'shell'
  | 'layout'
  | 'world'
  | 'control'
  | 'config'
  | 'windows'
>;

/** Joins a recording in progress; a page that refuses is logged and tried again on its next load. */
export function joinRecording(recorder: AppServices['recorder'], view: TabView): Promise<unknown> {
  return Promise.resolve(recorder.joinIfRecording(view)).catch((err: Error) =>
    console.error('[recording]', err.message),
  );
}

/**
 * The new tab loads the page the way the page asked for it. A form that posts
 * into a new window (a SAML single sign-on, say) sends a body, and servers check
 * the referrer: reloading the URL with a bare GET lost both, and such links
 * answered 403 Forbidden.
 */
export function loadOptionsFor({ referrer, postBody }: Pick<HandlerDetails, 'referrer' | 'postBody'>): LoadURLOptions {
  const options: LoadURLOptions = {};
  if (referrer?.url) options.httpReferrer = referrer;
  if (postBody?.data?.length) {
    const type = postBody.boundary ? `${postBody.contentType}; boundary=${postBody.boundary}` : postBody.contentType;
    Object.assign(options, { postData: postBody.data, extraHeaders: `Content-Type: ${type}` });
  }
  return options;
}

/** Windows the page opens, sign-in popups it is allowed, and its context menu. */
export class TabWindows {
  /** The tabs, the persona's partition, protection and the recording. */
  private readonly deps: TabWindowsDeps;
  /** The page's right-click menu. */
  private readonly menu: ContextMenu;

  /** `deps` are the main-process services (see services.ts). */
  constructor(deps: TabWindowsDeps) {
    this.deps = deps;
    this.menu = new ContextMenu(deps);
  }

  /** Wires the window-open handler, popup adoption and the context menu on a new tab. */
  wire(tab: Tab): void {
    const contents = tab.view.webContents;
    this.deps.externalApps.wire(contents);
    contents.setWindowOpenHandler((details) => this.open(details, tab));
    contents.on('did-create-window', (childWindow, details) => this.adoptPopup(childWindow, tab, details.url));
    contents.on('context-menu', (_e, params) => this.menu.show(tab.view, params));
  }

  /**
   * An anonymous target="_blank" becomes a tab, which is what a person wants.
   * A sign-in popup and any window the page named stay real windows, because the
   * page holds on to what `window.open` gave it. Those are then adopted as tabs
   * (adoptPopup) so an agent can still list, switch to and drive them.
   */
  private open(details: HandlerDetails, opener: Tab): WindowOpenHandlerResponse {
    if (this.deps.externalApps.request(details.url, opener.view.webContents)) return { action: 'deny' };
    const popup = this.popupOptions(details, opener);
    if (popup) return popup;
    // A page cannot load a file: address itself, but a tab the app opens for it could: the page must not get one that way.
    if (!LOCAL_FILE.test(details.url)) this.openSibling(details, opener);
    return { action: 'deny' };
  }

  /** Ordinary links inherit the exact opener session, including ephemeral contexts. */
  private openSibling(details: HandlerDetails, opener: Tab): void {
    const id = this.deps.tabs.createTab(details.url, true, loadOptionsFor(details), opener.view.webContents.session);
    this.markOpener(id, opener);
  }
  /** Preserve sign-in and named windows with the opener's persona partition. */
  private popupOptions(details: HandlerDetails, opener: Tab): WindowOpenHandlerResponse | null {
    const webPreferences = { session: opener.view.webContents.session };
    if (isAuthPopup(details.url, details.features))
      return { action: 'allow', overrideBrowserWindowOptions: { ...AUTH_POPUP_SIZE, webPreferences } };
    if (opensNamedWindow(details.frameName))
      return { action: 'allow', overrideBrowserWindowOptions: { webPreferences } };
    return null;
  }
  /** Notes which tab opened a tab, so what a test run's tabs open closes with them. */
  private markOpener(id: number, opener: Tab): void {
    const tab: Tab | undefined = this.deps.tabs.list.find((t: Tab) => t.id === id);
    if (tab && opener) tab.openerId = opener.id;
  }

  /** Protocol interception is installed before a popup can navigate. */
  private protectPopup(window: BrowserWindow): void {
    this.followPopupFocus(window);
    this.deps.externalApps.wire(window.webContents);
    this.deps.protection.protectPopup(window);
  }
  /** Keep app-level permission prompts and agent discovery on the focused popup's browser window. */
  private followPopupFocus(window: BrowserWindow): void {
    if (!this.deps.windows) return;
    window.on('focus', () => {
      const owner = this.deps.windows?.ownerOfContents(window.webContents);
      if (owner && this.deps.control.snapshot().interactive) this.deps.windows?.select(owner);
    });
  }
  /** Protects a window the page opened, and puts it on the tab list so it can be driven. */
  private adoptPopup(childWindow: BrowserWindow, opener: Tab, url: string): void {
    this.protectPopup(childWindow);
    this.deps.tabs.adoptWindow?.(childWindow);
    // A sign-in popup is part of the task: what the person types there is recorded too.
    const adopted: Tab | undefined = this.deps.tabs.list.find((t: Tab) => t.window === childWindow);
    if (adopted) {
      adopted.openerId = opener.id;
      joinRecording(this.deps.recorder, adopted.view);
    }
    if (adopted) new GmailPopup(this.deps, childWindow, opener.id, url);
  }
}
