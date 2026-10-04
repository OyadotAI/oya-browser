/**
 * Windows a tab's page opens, and its right-click menu. An anonymous
 * target="_blank" becomes a tab; a sign-in popup and a window the page named
 * stay real windows, protected and adopted onto the tab list.
 */
import type { BrowserWindow, HandlerDetails, LoadURLOptions, WindowOpenHandlerResponse } from 'electron';
import type { AppServices } from '../app/services.ts';
import { isAuthPopup, opensNamedWindow } from './auth-popup.ts';
import { ContextMenu } from './context-menu.ts';
import { AUTH_POPUP_SIZE, LOCAL_FILE } from './constants.ts';
import type { Tab, TabView } from './types.ts';

/** The services a tab's windows and menu use. */
export type TabWindowsDeps = Pick<
  AppServices,
  'tabs' | 'persona' | 'protection' | 'recorder' | 'electron' | 'shell' | 'layout' | 'world' | 'control' | 'config'
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
    contents.setWindowOpenHandler((details) => this.open(details, tab));
    contents.on('did-create-window', (childWindow) => this.adoptPopup(childWindow));
    contents.on('context-menu', (_e, params) => this.menu.show(tab.view, params));
  }

  /**
   * An anonymous target="_blank" becomes a tab, which is what a person wants.
   * A sign-in popup and any window the page named stay real windows, because the
   * page holds on to what `window.open` gave it. Those are then adopted as tabs
   * (adoptPopup) so an agent can still list, switch to and drive them.
   */
  private open(details: HandlerDetails, opener: Tab): WindowOpenHandlerResponse {
    const webPreferences = { partition: this.deps.persona.partitionName() };
    if (isAuthPopup(details.url, details.features))
      return { action: 'allow', overrideBrowserWindowOptions: { ...AUTH_POPUP_SIZE, webPreferences } };
    if (opensNamedWindow(details.frameName))
      return { action: 'allow', overrideBrowserWindowOptions: { webPreferences } };
    // A page cannot load a file: address itself, but a tab the app opens for it could: the page must not get one that way.
    if (!LOCAL_FILE.test(details.url))
      this.markOpener(this.deps.tabs.createTab(details.url, true, loadOptionsFor(details)), opener);
    return { action: 'deny' };
  }

  /** Notes which tab opened a tab, so what a test run's tabs open closes with them. */
  private markOpener(id: number, opener: Tab): void {
    const tab: Tab | undefined = this.deps.tabs.list.find((t: Tab) => t.id === id);
    if (tab && opener) tab.openerId = opener.id;
  }

  /** Protects a window the page opened, and puts it on the tab list so it can be driven. */
  private adoptPopup(childWindow: BrowserWindow): void {
    this.deps.protection.protectPopup(childWindow);
    this.deps.tabs.adoptWindow?.(childWindow);
    // A sign-in popup is part of the task: what the person types there is recorded too.
    const adopted: Tab | undefined = this.deps.tabs.list.find((t: Tab) => t.window === childWindow);
    if (adopted) joinRecording(this.deps.recorder, adopted.view);
  }
}
