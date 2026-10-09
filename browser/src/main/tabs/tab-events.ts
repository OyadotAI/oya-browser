/**
 * What a tab listens to: its load state, its address, title and icon, windows
 * it tries to open, and its right-click menu. Wired once, when the tab is made,
 * along with its protection before the first page (tab-protector.ts).
 */
import { privateSession } from '../native-contexts/index.ts';
import type { AppServices } from '../app/services.ts';
import { watchContents } from '../observe/install.ts';
import { trackFrameSessions } from '../recording/frame-sessions.ts';
import { ERR_ABORTED } from './constants.ts';
import { wireHome } from './home.ts';
import { wireFavicon } from './favicon.ts';
import { TabProtector } from './tab-protector.ts';
import { TabWindows, joinRecording, type TabWindowsDeps } from './tab-windows.ts';
import type { Tab, TabView } from './types.ts';

/** The services a tab's listeners use. */
export type TabEventsDeps = TabWindowsDeps & Pick<AppServices, 'shortcuts' | 'shield' | 'observer' | 'library'>;

/**
 * Makes view-source pages readable (forces the light theme). This text runs
 * in the page, so it is kept exactly as it has always been.
 */
export const VIEW_SOURCE_LIGHT = `
        document.documentElement.style.cssText = 'background:#fff!important;color:#000!important;color-scheme:light!important';
        document.body.style.cssText = 'background:#fff!important;color:#000!important';
        const s = document.createElement('style');
        s.textContent = '*, *::before, *::after { color-scheme: light !important; } body, html, .line-content, .line-number, td, tr, table { background-color: #fff !important; color: #000 !important; } a { color: #00e !important; }';
        document.head.appendChild(s);
      `;

/** Wires every listener a tab gets, and starts its protection. */
export class TabEvents {
  /** The tabs, the shell's shortcuts and shield, the recording and protection. */
  private readonly deps: TabEventsDeps;
  /** Protects each new tab before its first page. */
  private readonly protector: TabProtector;
  /** Windows the page opens, and its context menu. */
  private readonly windows: TabWindows;

  /** `deps` are the main-process services (see services.ts). */
  constructor(deps: TabEventsDeps) {
    this.deps = deps;
    this.protector = new TabProtector(deps);
    this.windows = new TabWindows(deps);
  }

  /** Wires every listener on a new tab; returns the promise that settles once it is protected (or given up on). */
  wire(tab: Tab): Promise<void> {
    this.wireState(tab);
    this.wireLibrary(tab);
    const tabReady = this.protector.protect(tab);
    this.wirePage(tab, tabReady);
    this.windows.wire(tab);
    return tabReady;
  }

  /** What comes before protection: recording, load state, failures, the start page and the icon, in that order. */
  private wireState(tab: Tab): void {
    this.wireRecording(tab);
    this.wireLoadState(tab);
    this.wireFailures(tab);
    this.wireRendererLoss(tab);
    wireHome(this.deps.tabs, tab);
    wireFavicon(this.deps.tabs, tab);
  }

  /** Shortcuts, focus, and the loading / failed state the tab strip shows. */
  private wireLoadState(tab: Tab): void {
    const contents = tab.view.webContents;
    this.deps.shortcuts.install(contents);
    contents.on('focus', () => this.deps.shield.keepFocusOnShell());
    contents.on('did-start-loading', () => {
      // A tab that could not be protected keeps saying so: nothing it starts will load.
      if (tab.protection !== 'failed') tab.loadError = null;
      this.deps.tabs.sendTabList();
    });
    contents.on('did-stop-loading', () => this.deps.tabs.sendTabList());
  }

  /** Puts an error on the tab and stops its spinner. */
  private showTabError(tab: Tab, message: string): void {
    tab.navigationPending = false;
    tab.loadError = message;
    this.deps.tabs.sendTabList();
  }

  /** A dead renderer's recording channel died with it; forgotten, the reload's page arms a new one. */
  private wireRendererLoss(tab: Tab): void {
    tab.view.webContents.on('render-process-gone', () => this.deps.recorder.channels.forget(tab.view));
  }

  /** A page that could not load, or a renderer that died, is shown on the tab. */
  private wireFailures(tab: Tab): void {
    const failed = (message: string): void => this.showTabError(tab, message);
    tab.view.webContents.on('did-fail-load', (_event, code, description, _url, mainFrame) => {
      if (!mainFrame || code === ERR_ABORTED) return;
      failed(`Page could not load: ${description}. Try Reload.`);
    });
    tab.view.webContents.on('render-process-gone', (_event, details) => {
      failed(`Page renderer stopped (${details.reason}). Reload to recover.`);
    });
  }

  /** A finished load, the address, the title, and joining a recording in progress. */
  private wirePage(tab: Tab, tabReady: Promise<void>): void {
    const contents = tab.view.webContents;
    contents.on('did-finish-load', () => this.pageLoaded(tab.view));
    const updateUrl = (_e: unknown, u: string): void => this.deps.tabs.urlChanged(tab, u);
    contents.on('did-navigate', updateUrl);
    contents.on('did-navigate-in-page', (event, url, mainFrame) => mainFrame && updateUrl(event, url));
    // New tabs join an active recording before the user can interact with them; one never protected has nothing to record.
    tabReady.then(() => tab.protection === 'protected' && joinRecording(this.deps.recorder, tab.view));
    this.wireTitle(tab);
    if (this.deps.observer) watchContents(this.deps.observer, contents);
  }

  /**
   * What a recording needs from the tab: its cross-site iframes, tracked before
   * protection attaches to them so a later recording finds them, and a page check
   * when a person's action moves the page (recording/outcomes.ts).
   */
  private wireRecording(tab: Tab): void {
    trackFrameSessions(tab.view);
    const contents = tab.view.webContents;
    contents.on('did-navigate', (_e, u) => this.deps.recorder.pageReached?.(tab.id, u));
    contents.on(
      'did-navigate-in-page',
      (_e, u, isMainFrame) => isMainFrame && this.deps.recorder.pageReached?.(tab.id, u),
    );
  }

  /**
   * The tab's title. A page with no <title> (about:blank) never fires
   * page-title-updated, and the tab kept the previous page's name; reading it
   * again on every load fixes that, since getTitle() falls back to the address.
   */
  private wireTitle(tab: Tab): void {
    const contents = tab.view.webContents;
    contents.on('page-title-updated', (_e, title) => this.deps.tabs.titleChanged(tab, title));
    contents.on('did-finish-load', () => this.deps.tabs.titleChanged(tab, contents.getTitle()));
  }

  /** Only successful main-frame visits enter the local library; subframes never replace the tab address. */
  private wireLibrary(tab: Tab): void {
    if (privateSession(tab.view.webContents.session)) return;
    const contents = tab.view.webContents;
    const visit = (): void => this.deps.library.visit(contents.getURL(), contents.getTitle());
    contents.on('did-finish-load', visit);
    contents.on('did-navigate-in-page', (_event, _url, mainFrame) => mainFrame && visit());
  }

  /** Loads the analyzer, and lightens view-source pages. */
  private pageLoaded(view: TabView): void {
    this.deps.protection.injectScripts(view, true);
    // A tab that could not be armed, or whose renderer died, joins the recording again here.
    joinRecording(this.deps.recorder, view);
    const currentUrl = view.webContents.getURL();
    if (currentUrl.startsWith('view-source:')) {
      view.webContents.executeJavaScript(VIEW_SOURCE_LIGHT, true).catch(() => {});
    }
  }
}
