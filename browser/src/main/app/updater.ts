/**
 * Squirrel.Mac stages the new bundle and swaps it when the app quits, so an
 * update never interrupts a browsing session, it is simply there next launch.
 * Someone who wants it sooner gets a Restart button, wired to install-update.
 *
 * Cloud browsers are pinned to the snapshot they were provisioned from; one
 * that upgraded itself mid-run would no longer be the build the fleet expects.
 */
import type { AppUpdater, ProgressInfo, UpdateInfo } from 'electron-updater';
import type { AppServices } from './services.ts';
import { UPDATE_CHECK_INTERVAL_MS, UPDATE_FIRST_CHECK_MS } from './constants.ts';

/** The services the updater uses, and what runs before an update restart. */
interface Deps extends Pick<AppServices, 'electron' | 'shell'> {
  /** Writes the cookie jar to disk before the app relaunches into the update. */
  beforeInstall?: () => unknown;
}

/** Download progress, with the version when the updater names it. */
interface Progress extends Pick<ProgressInfo, 'percent'> {
  /** The version being downloaded. */
  version?: string;
}

/** Loads electron-updater's autoUpdater. */
export type LoadUpdater = () => Promise<AppUpdater>;

/** What the toolbar shows. */
export interface UpdateState {
  /** 'current', 'unsupported', 'checking', 'available', 'downloading', 'ready' or 'error'. */
  state: string;
  /** The version on offer, or null. */
  version: string | null;
  /** This app's version. */
  current?: string;
  /** Download progress, in whole percent. */
  percent?: number;
  /** Why a check failed. */
  message?: string;
  /** Lets the shell read it as a payload. */
  [key: string]: unknown;
}

/** Loaded only once a packaged desktop starts updating, so dev runs and cloud browsers never load it at all. */
const loadElectronUpdater: LoadUpdater = async () => (await import('electron-updater')).autoUpdater;

/** The message of whatever was thrown. */
const messageOf = (err: unknown): string => (err as Error | undefined)?.message || String(err);

/** The auto-updater and the toolbar's view of it. */
export class Updater {
  /** What the toolbar shows. */
  state: UpdateState = { state: 'current', version: null };
  /** electron-updater's autoUpdater, once started in a packaged app. */
  private autoUpdater: AppUpdater | null = null;
  /** The version already announced with an OS notification. */
  private notifiedVersion: string | null = null;
  /** The main-process services. */
  private readonly deps: Deps;
  /** Loads electron-updater (a fake in tests). */
  private readonly loadUpdater: LoadUpdater;

  /** `deps` gives Electron, the shell and the step before an update restart; `loadUpdater` is for tests. */
  constructor(deps: Deps, loadUpdater: LoadUpdater = loadElectronUpdater) {
    this.deps = deps;
    this.loadUpdater = loadUpdater;
  }

  /** This app's version. */
  version(): string {
    return this.deps.electron.app.getVersion();
  }

  /** Single source of truth for the toolbar, so a late-loading window still learns. */
  private setUpdateState(next: UpdateState): void {
    this.state = { ...next, current: this.version() };
    this.deps.shell.send('update-status', this.state);
  }

  /** Starts checking for updates; dev runs and cloud browsers only report that they cannot. */
  async start(): Promise<void> {
    if (!this.deps.electron.app.isPackaged || process.env.OYA_DOCKER === 'true') {
      this.setUpdateState({ state: 'unsupported', version: null });
      return;
    }
    const autoUpdater = await this.load();
    this.listen(autoUpdater);
    this.schedule(autoUpdater);
  }

  /** Loads electron-updater: downloads automatically, installs on quit. */
  private async load(): Promise<AppUpdater> {
    const autoUpdater = await this.loadUpdater();
    autoUpdater.autoDownload = true;
    autoUpdater.autoInstallOnAppQuit = true;
    // The server counts update checks by the version asking; nothing else rides along.
    autoUpdater.requestHeaders = { 'X-Oya-Version': this.version() };
    this.autoUpdater = autoUpdater;
    return autoUpdater;
  }

  /** Mirrors each updater event into the toolbar state. */
  private listen(autoUpdater: AppUpdater): void {
    autoUpdater.on('checking-for-update', () => this.setUpdateState({ state: 'checking', version: null }));
    autoUpdater.on('update-not-available', () => this.setUpdateState({ state: 'current', version: null }));
    autoUpdater.on('update-available', (info) => this.available(info));
    autoUpdater.on('download-progress', (progress) => this.progress(progress));
    autoUpdater.on('update-downloaded', (info) => this.downloaded(info));
    autoUpdater.on('error', (err) => this.failed(err));
  }

  /** A newer version exists: show it, and say so once at the OS level. */
  private available(info: UpdateInfo): void {
    console.log('[update] available:', info.version);
    this.setUpdateState({ state: 'available', version: info.version });
    // The window is often not the thing being looked at, so say it once at the
    // OS level too. Only on the transition, never on the periodic re-checks.
    if (this.deps.electron.Notification.isSupported() && this.notifiedVersion !== info.version) {
      this.notify(info.version);
    }
  }

  /** The OS notification for a newly available version. */
  private notify(version: string): void {
    this.notifiedVersion = version;
    new this.deps.electron.Notification({
      title: `Oya Browser ${version} is available`,
      body: `You are on ${this.version()}. It installs when you quit, or restart now from the toolbar.`,
      silent: true,
    }).show();
  }

  /** Download progress, in whole percent. */
  private progress({ percent, version }: Progress): void {
    this.setUpdateState({ state: 'downloading', version: version || this.state.version, percent: Math.round(percent) });
  }

  /** The update is staged and applies on quit. */
  private downloaded(info: UpdateInfo): void {
    console.log('[update] ready, applies on quit:', info.version);
    this.setUpdateState({ state: 'ready', version: info.version });
  }

  /** A missed check is not worth interrupting anyone over; the next one retries. */
  private failed(err: unknown): void {
    console.log('[update] check failed:', (err as Error | undefined)?.message || err);
    this.setUpdateState({ state: 'error', version: null, message: messageOf(err) });
  }

  /** The first check once the window settles, then one every interval. */
  private schedule(autoUpdater: AppUpdater): void {
    const check = (): void => void autoUpdater.checkForUpdates().catch(() => {});
    setTimeout(check, UPDATE_FIRST_CHECK_MS).unref?.(); // let the first window settle
    setInterval(check, UPDATE_CHECK_INTERVAL_MS).unref?.();
  }

  /** The toolbar's "check now". */
  async checkNow(): Promise<UpdateState> {
    if (!this.autoUpdater) return this.state;
    try {
      await this.autoUpdater.checkForUpdates();
    } catch (e) {
      this.setUpdateState({ state: 'error', version: null, message: messageOf(e) });
    }
    return this.state;
  }

  /** Restarts into a staged update; false when none is ready. */
  install(): boolean {
    const autoUpdater = this.autoUpdater;
    if (!autoUpdater || this.state.state !== 'ready') return false;
    // Nothing else gets to run after this, it relaunches the app, so the jar goes to disk first.
    const install = (): void => autoUpdater.quitAndInstall();
    setImmediate(() => Promise.resolve(this.deps.beforeInstall?.()).then(install, install));
    return true;
  }
}
