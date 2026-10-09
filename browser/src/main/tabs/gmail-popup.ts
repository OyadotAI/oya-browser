/** Return a completed Google sign-in to browser chrome without interrupting OAuth callbacks. */
import type { BrowserWindow } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Tab } from './types.ts';
import { GoogleAppLoad } from './google-app-load.ts';

/** The normal tab path supplies the same persona, cookies and protection as every other page. */
type Deps = Pick<AppServices, 'tabs' | 'shell' | 'control'>;
/** Exact HTTPS origins prevent a lookalike or callback URL from triggering the handoff. */
export function isGoogleSignIn(raw: string): boolean {
  const url = URL.parse(raw);
  return !!url && url.origin === 'https://accounts.google.com' && !url.username && !url.password;
}
/** Only the actual mailbox, never an OAuth callback or a link mentioning Gmail. */
export function isGmailMailbox(raw: string): boolean {
  const url = URL.parse(raw);
  return (
    !!url &&
    url.origin === 'https://mail.google.com' &&
    url.pathname.startsWith('/mail/') &&
    !url.username &&
    !url.password
  );
}
/** Completed first-party app documents, not arbitrary Google or OAuth callback pages. */
export function isGoogleAppDestination(raw: string): boolean {
  if (isGmailMailbox(raw)) return true;
  const url = URL.parse(raw);
  if (!url || url.username || url.password) return false;
  return (
    url.origin === 'https://calendar.google.com' &&
    (url.pathname === '/calendar' || url.pathname.startsWith('/calendar/'))
  );
}
/** A staged app must remain on the same approved product as its verified popup. */
function matchesDestination(destination: string, source: string): boolean {
  return isGoogleAppDestination(destination) && new URL(destination).origin === new URL(source).origin;
}
/** A single popup's completion watcher; a failed handoff leaves the original login intact. */
export class GmailPopup {
  /** Browser services, not page-provided callbacks. */
  private readonly deps: Deps;
  /** The live sign-in window. */
  private readonly window: BrowserWindow;
  /** Where the sign-in started, for normal tab return ordering. */
  private readonly openerId: number;
  /** A popup must actually visit Google sign-in first. */
  private signingIn: boolean;
  /** At most one automatic handoff; duplicate load events cannot multiply tabs. */
  private attempted = false;
  /** Observe committed pages only; never cancel or replay a verification request. */
  constructor(deps: Deps, window: BrowserWindow, openerId: number, initialUrl: string) {
    this.deps = deps;
    this.window = window;
    this.openerId = openerId;
    this.signingIn = isGoogleSignIn(initialUrl);
    this.observe();
  }
  /** Observe the popup without cancelling or replaying its navigation. */
  private observe(): void {
    const window = this.window;
    window.webContents.on('did-navigate', (_event, url) => {
      this.signingIn ||= isGoogleSignIn(url);
    });
    window.webContents.on('dom-ready', () => void this.loaded());
    window.webContents.on('did-finish-load', () => void this.loaded());
  }
  /** Verification remains a real window until a supported Google app document has finished loading. */
  private async loaded(): Promise<void> {
    if (this.window.isDestroyed() || this.attempted || !this.deps.control.snapshot().interactive) return;
    const url = this.window.webContents.getURL();
    this.signingIn ||= isGoogleSignIn(url);
    if (!this.signingIn || !isGoogleAppDestination(url)) return;
    this.attempted = true;
    await this.handoff(url).catch(() => {});
  }
  /** Stage the app in the background; a failure must not destroy the verified session. */
  private async handoff(url: string): Promise<void> {
    const tab = this.deps.tabs.find(this.deps.tabs.createTab(url, false, undefined, this.window.webContents.session));
    if (!tab) return;
    tab.openerId = this.openerId;
    const load = new GoogleAppLoad(tab, () => this.canFinish(tab, url));
    await load
      .wait()
      .then(() => this.finish(tab, url))
      .catch(() => this.discard(tab));
  }
  /** A staged failure is disposable; the original verified popup is not. */
  private discard(tab: Tab): void {
    if (this.deps.tabs.find(tab.id)) this.deps.tabs.closeTab(tab.id, { keepOne: false });
  }
  /** Only a protected, loaded app can replace the popup; changed or closed windows are left alone. */
  private finish(tab: Tab, source: string): void {
    const { tabs, control, shell } = this.deps;
    if (!this.canFinish(tab, source) || !control.snapshot().interactive) return this.discard(tab);
    const popup = tabs.list.find((entry) => entry.window === this.window);
    if (tabs.activeTabId === popup?.id) tabs.activateTab(tab.id);
    if (tabs.activeTabId === tab.id) shell.window?.show();
    this.window.close();
  }
  /** Re-check both windows after asynchronous protection and loading, including redirects back to sign-in. */
  private canFinish(tab: Tab, source: string): boolean {
    return (
      !this.window.isDestroyed() &&
      this.window.webContents.getURL() === source &&
      this.deps.tabs.find(tab.id) === tab &&
      tab.protection === 'protected' &&
      !tab.loadError &&
      matchesDestination(tab.view.webContents.getURL(), source)
    );
  }
}
