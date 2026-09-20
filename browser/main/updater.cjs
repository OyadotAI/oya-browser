/**
 * Squirrel.Mac stages the new bundle and swaps it when the app quits, so an
 * update never interrupts a browsing session — it is simply there next launch.
 * Someone who wants it sooner gets a Restart button, wired to install-update.
 *
 * Cloud browsers are pinned to the snapshot they were provisioned from; one
 * that upgraded itself mid-run would no longer be the build the fleet expects.
 */
const { app, Notification } = require('electron');
const { UPDATE_CHECK_INTERVAL_MS, UPDATE_FIRST_CHECK_MS } = require('./constants.cjs');

/** The auto-updater and the toolbar's view of it. */
class Updater {
  /** `sendToRenderer` updates the toolbar. */
  constructor(sendToRenderer) {
    /** Pushes state to the shell. */
    this.sendToRenderer = sendToRenderer;
    /** electron-updater's autoUpdater, once started in a packaged app. */
    this.autoUpdater = null;
    /** What the toolbar shows. */
    this.state = { state: 'current', version: null };
    /** The version already announced with an OS notification. */
    this.notifiedVersion = null;
  }

  /** Single source of truth for the toolbar, so a late-loading window still learns. */
  setUpdateState(next) {
    this.state = { ...next, current: app.getVersion() };
    this.sendToRenderer('update-status', this.state);
  }

  /** Starts checking for updates; dev runs and cloud browsers only report that they cannot. */
  start() {
    if (!app.isPackaged || process.env.OYA_DOCKER === 'true') {
      this.setUpdateState({ state: 'unsupported', version: null });
      return;
    }
    this.load();
    this.listen();
    this.schedule();
  }

  /** Loads electron-updater: downloads automatically, installs on quit. */
  load() {
    // Required lazily so dev runs and cloud browsers never load it at all.
    ({ autoUpdater: this.autoUpdater } = require('electron-updater'));
    this.autoUpdater.autoDownload = true;
    this.autoUpdater.autoInstallOnAppQuit = true;
  }

  /** Mirrors each updater event into the toolbar state. */
  listen() {
    const on = (event, fn) => this.autoUpdater.on(event, fn);
    on('checking-for-update', () => this.setUpdateState({ state: 'checking', version: null }));
    on('update-not-available', () => this.setUpdateState({ state: 'current', version: null }));
    on('update-available', (info) => this.available(info));
    on('download-progress', (progress) => this.progress(progress));
    on('update-downloaded', (info) => this.downloaded(info));
    on('error', (err) => this.failed(err));
  }

  /** A newer version exists: show it, and say so once at the OS level. */
  available(info) {
    console.log('[update] available:', info.version);
    this.setUpdateState({ state: 'available', version: info.version });
    // The window is often not the thing being looked at, so say it once at the
    // OS level too. Only on the transition — never on the periodic re-checks.
    if (Notification.isSupported() && this.notifiedVersion !== info.version) this.notify(info.version);
  }

  /** The OS notification for a newly available version. */
  notify(version) {
    this.notifiedVersion = version;
    new Notification({
      title: `Oya Browser ${version} is available`,
      body: `You are on ${app.getVersion()}. It installs when you quit, or restart now from the toolbar.`,
      silent: true,
    }).show();
  }

  /** Download progress, in whole percent. */
  progress({ percent, version }) {
    this.setUpdateState({ state: 'downloading', version: version || this.state.version, percent: Math.round(percent) });
  }

  /** The update is staged and applies on quit. */
  downloaded(info) {
    console.log('[update] ready, applies on quit:', info.version);
    this.setUpdateState({ state: 'ready', version: info.version });
  }

  /** A missed check is not worth interrupting anyone over; the next one retries. */
  failed(err) {
    console.log('[update] check failed:', err?.message || err);
    this.setUpdateState({ state: 'error', version: null, message: err?.message || String(err) });
  }

  /** The first check once the window settles, then one every interval. */
  schedule() {
    const check = () => this.autoUpdater.checkForUpdates().catch(() => {});
    setTimeout(check, UPDATE_FIRST_CHECK_MS).unref?.(); // let the first window settle
    setInterval(check, UPDATE_CHECK_INTERVAL_MS).unref?.();
  }

  /** The toolbar's "check now". */
  async checkNow() {
    if (!this.autoUpdater) return this.state;
    try {
      await this.autoUpdater.checkForUpdates();
    } catch (e) {
      this.setUpdateState({ state: 'error', version: null, message: e?.message || String(e) });
    }
    return this.state;
  }

  /** Restarts into a staged update; false when none is ready. */
  install() {
    if (!this.autoUpdater || this.state.state !== 'ready') return false;
    // Nothing else gets to run after this — it relaunches the app.
    setImmediate(() => this.autoUpdater.quitAndInstall());
    return true;
  }
}

/** `handle` registers an IPC handler; `sendToRenderer` updates the toolbar. */
function createUpdater({ handle, sendToRenderer }) {
  const updater = new Updater(sendToRenderer);
  handle('check-for-updates', () => updater.checkNow());
  handle('get-update-status', () => updater.state);
  handle('install-update', () => updater.install());
  handle('get-version', () => app.getVersion());
  return { startAutoUpdate: () => updater.start() };
}

module.exports = { createUpdater };
