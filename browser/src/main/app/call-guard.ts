/**
 * Asks before a window close or a quit ends a call. Closing a window tears
 * down its tabs at once, and on macOS closing the last one quits, so a stray
 * red button or Cmd+Q used to drop a meeting with no warning, and looked like
 * a crash. Also the one owner of "this quit is going through": a quit the
 * person cancels must not leave every window closable past the agent's hold.
 */
import type { BrowserWindow, Event, MessageBoxOptions } from 'electron';
import type { AppServices } from './services.ts';
import type { Tab } from '../tabs/types.ts';
import { endCallPrompt } from './media-prompts.ts';
import { END_CALL_CHOICE, SHUTDOWN_GRACE_MS } from './constants.ts';

/** The device gate that knows which documents hold a device, the windows, and the dialog. */
type Deps = Pick<AppServices, 'electron' | 'mediaPermissions' | 'windows'>;

/** The end-call dialog, and whether a quit has been committed. */
export class CallGuard {
  /** Set once a quit goes through: confirmed, no call live, or an update restart. Never unset. */
  private quitCommitted = false;
  /** The OS asked to log out or power off. Lapses if no quit follows, as when another app cancels the logout. */
  private osQuitting = false;
  /** The main-process services. */
  private readonly deps: Deps;
  /** macOS only: elsewhere a dialog could stand in front of a Windows logoff nobody has tested. */
  private readonly platform: string;
  /** Windows whose close the person confirmed, so the repeated close goes through. */
  private readonly confirmed = new WeakSet<BrowserWindow>();
  /** Dismisses an open dialog when the OS shuts down or logs out; replaced after, so later dialogs still show. */
  private shuttingDown = new AbortController();
  /** One dialog at a time: a second Cmd+Q or red button while it is up asks nothing. */
  private pending = false;

  /** `deps` is the main-process services (see src/main/main.ts); `platform` is for tests. */
  constructor(deps: Deps, platform = process.platform) {
    this.deps = deps;
    this.platform = platform;
  }

  /** Whether a quit is going through: nothing is asked, and the agent's close hold gives way. */
  get committed(): boolean {
    return this.quitCommitted || this.osQuitting;
  }

  /** This quit goes through; nothing after it is asked about. */
  commit(): void {
    this.quitCommitted = true;
  }

  /** The OS is logging out or powering off: close any dialog and let its quit through, for a while. */
  shutdown(): void {
    this.osQuitting = true;
    this.shuttingDown.abort();
    this.shuttingDown = new AbortController();
    setTimeout(() => void (this.osQuitting = false), SHUTDOWN_GRACE_MS).unref?.();
  }

  /** The hostnames whose tab still holds a device, each once. */
  sites(tabs: Tab[]): string[] {
    if (this.platform !== 'darwin') return [];
    const { gate } = this.deps.mediaPermissions;
    const live = tabs.filter((tab) => gate.inCall(tab.view.webContents));
    const hosts = live.map((tab) => URL.parse(tab.view.webContents.getURL())?.hostname ?? '');
    return [...new Set(hosts.filter(Boolean))];
  }

  /** Holds a window close that would end a call, and asks. Answers whether it held. */
  holdClose(window: BrowserWindow, tabs: Tab[], event: Event): boolean {
    if (this.committed || this.confirmed.has(window)) return false;
    const hosts = this.sites(tabs);
    if (!hosts.length) return false;
    event.preventDefault();
    void this.ask(window, endCallPrompt('close', hosts)).then((yes) => yes && this.closeAnyway(window));
    return true;
  }

  /** The person chose Close Window. */
  private closeAnyway(window: BrowserWindow): void {
    if (window.isDestroyed()) return;
    // Only for this close: if something else holds it (an agent took a tab meanwhile), the next close asks again.
    this.confirmed.add(window);
    window.close();
    this.confirmed.delete(window);
  }

  /** Holds a quit while any window has a call, and asks in front of it. Otherwise commits the quit. */
  holdQuit(event: Event): boolean {
    const tab = this.committed ? undefined : this.callTab();
    if (!tab) {
      this.commit();
      return false;
    }
    event.preventDefault();
    void this.askToQuit(tab);
    return true;
  }

  /** The first tab, in any window, whose document holds a device. */
  private callTab(): Tab | undefined {
    return this.deps.windows?.allTabs().find((tab) => this.sites([tab]).length);
  }

  /** Brings the call forward so the person sees which one it means, then asks. */
  private async askToQuit(tab: Tab): Promise<void> {
    const scope = this.deps.windows?.owner(tab.id);
    scope?.shell.window?.show();
    scope?.tabs.activateTab(tab.id);
    const hosts = this.sites(this.deps.windows?.allTabs() ?? []);
    if (!(await this.ask(scope?.shell.window, endCallPrompt('quit', hosts)))) return;
    this.commit();
    this.deps.electron.app.quit();
  }

  /** Whether the person chose to end the call. A second ask while one is up, or a failed dialog, is a no. */
  private ask(window: BrowserWindow | null | undefined, options: MessageBoxOptions): Promise<boolean> {
    if (this.pending) return Promise.resolve(false);
    this.pending = true;
    return this.show(window, options)
      .then(
        (answer) => answer.response === END_CALL_CHOICE,
        () => false,
      )
      .finally(() => void (this.pending = false));
  }

  /** A sheet on the call's window when there is one; never a null parent. Shutting down closes it as a Cancel. */
  private show(window: BrowserWindow | null | undefined, options: MessageBoxOptions) {
    const { dialog } = this.deps.electron;
    const withSignal = { ...options, signal: this.shuttingDown.signal };
    return window && !window.isDestroyed()
      ? dialog.showMessageBox(window, withSignal)
      : dialog.showMessageBox(withSignal);
  }
}
