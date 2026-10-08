/** Return a completed Google sign-in to browser chrome without interrupting OAuth callbacks. */
import type { BrowserWindow } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Tab } from './types.ts';

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
    window.webContents.on('did-navigate', (_event, url) => {
      this.signingIn ||= isGoogleSignIn(url);
    });
    window.webContents.on('did-finish-load', () => void this.loaded());
  }
  /** Verification remains a real window until a Gmail document has finished loading. */
  private async loaded(): Promise<void> {
    if (this.window.isDestroyed() || this.attempted || !this.deps.control.snapshot().interactive) return;
    const url = this.window.webContents.getURL();
    this.signingIn ||= isGoogleSignIn(url);
    if (!this.signingIn || !isGmailMailbox(url)) return;
    this.attempted = true;
    await this.handoff(url).catch(() => {});
  }
  /** Stage the mailbox in the background; a failure must not destroy the verified session. */
  private async handoff(url: string): Promise<void> {
    const { tabs } = this.deps;
    const tab = tabs.find(tabs.createTab(url, false));
    if (!tab) return;
    tab.openerId = this.openerId;
    await tab.ready?.then(() => this.finish(tab, url)).catch(() => this.discard(tab));
  }
  /** A staged failure is disposable; the original verified popup is not. */
  private discard(tab: Tab): void {
    if (this.deps.tabs.find(tab.id)) this.deps.tabs.closeTab(tab.id, { keepOne: false });
  }
  /** Only a protected, loaded mailbox can replace the popup; changed or closed windows are left alone. */
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
      isGmailMailbox(tab.view.webContents.getURL())
    );
  }
}
