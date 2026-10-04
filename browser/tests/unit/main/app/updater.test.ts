/**
 * Unit tests for the Updater (src/main/app/updater.ts) and its IPC channels
 * (src/main/ipc/updates.ts): the toolbar's update state, and why dev runs and
 * cloud browsers never update. Electron and electron-updater are faked.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { Updater } from '../../../../src/main/app/updater.ts';
import { UpdateHandlers } from '../../../../src/main/ipc/updates.ts';
import { UPDATE_FIRST_CHECK_MS, UPDATE_CHECK_INTERVAL_MS } from '../../../../src/main/app/constants.ts';

/** A fake electron-updater autoUpdater that counts its checks. */
function fakeAutoUpdater(): any {
  const autoUpdater: any = new EventEmitter();
  autoUpdater.checks = 0;
  autoUpdater.checkForUpdates = async () => {
    autoUpdater.checks++;
  };
  autoUpdater.quitAndInstall = () => (autoUpdater.installed = true);
  return autoUpdater;
}

describe('Updater', () => {
  let notifications: any[];
  let autoUpdater: any;
  let handlers: any;
  let statuses: any[];

  /** A packaged (or dev) app with the updater built and its IPC table at hand. */
  function setUp(isPackaged: boolean, beforeInstall?: () => Promise<unknown>): Updater {
    notifications = [];
    /** Records each OS notification shown. */
    class Notification {
      /** The options it was built with. */
      options: unknown;
      /** Keeps the options. */
      constructor(options: unknown) {
        this.options = options;
      }
      /** Records it. */
      show() {
        notifications.push(this.options);
      }
      /** Notifications work here. */
      static isSupported() {
        return true;
      }
    }
    statuses = [];
    const electron: any = { app: { isPackaged, getVersion: () => '1.0.0' }, Notification };
    const shell: any = { send: (channel: string, state: unknown) => statuses.push([channel, state]) };
    const updater = new Updater({ electron, shell, beforeInstall }, async () => autoUpdater);
    handlers = new UpdateHandlers({ updater } as any).handlers;
    return updater;
  }

  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout', 'setInterval', 'setImmediate'] });
    mock.method(console, 'log', () => {});
    autoUpdater = fakeAutoUpdater();
  });
  afterEach(() => {
    mock.timers.reset();
    mock.restoreAll();
  });

  it('answers the update IPC channels', () => {
    setUp(true);
    assert.deepEqual(Object.keys(handlers), [
      'check-for-updates',
      'get-update-status',
      'install-update',
      'get-version',
    ]);
    assert.equal(handlers['get-version'](), '1.0.0');
  });

  it('reports updates as unsupported in a dev run', async () => {
    await setUp(false).start();
    assert.deepEqual(statuses, [['update-status', { state: 'unsupported', version: null, current: '1.0.0' }]]);
    assert.deepEqual(await handlers['check-for-updates'](), statuses[0][1]);
    assert.equal(autoUpdater.checks, 0);
  });

  it('never updates a cloud browser, even when packaged', async () => {
    process.env.OYA_DOCKER = 'true';
    try {
      await setUp(true).start();
    } finally {
      delete process.env.OYA_DOCKER;
    }
    assert.equal(statuses[0][1].state, 'unsupported');
  });

  it('checks after the first window settles and then on an interval', async () => {
    await setUp(true).start();
    assert.equal(autoUpdater.checks, 0);
    mock.timers.tick(UPDATE_FIRST_CHECK_MS);
    assert.equal(autoUpdater.checks, 1);
    mock.timers.tick(UPDATE_CHECK_INTERVAL_MS);
    assert.equal(autoUpdater.checks, 2);
    assert.equal(autoUpdater.autoDownload, true);
    assert.deepEqual(autoUpdater.requestHeaders, { 'X-Oya-Version': '1.0.0' }, 'checks say which version asks');
    assert.equal(autoUpdater.autoInstallOnAppQuit, true);
  });

  it('mirrors each updater event into the toolbar state', async () => {
    await setUp(true).start();
    autoUpdater.emit('checking-for-update');
    autoUpdater.emit('update-available', { version: '2.0.0' });
    autoUpdater.emit('download-progress', { percent: 41.6 });
    autoUpdater.emit('update-downloaded', { version: '2.0.0' });
    autoUpdater.emit('error', new Error('offline'));
    assert.deepEqual(
      statuses.map(([, s]) => [s.state, s.version, s.percent ?? s.message]),
      [
        ['checking', null, undefined],
        ['available', '2.0.0', undefined],
        ['downloading', '2.0.0', 42],
        ['ready', '2.0.0', undefined],
        ['error', null, 'offline'],
      ],
    );
  });

  it('announces a new version at the OS level once, not on every re-check', async () => {
    await setUp(true).start();
    autoUpdater.emit('update-available', { version: '2.0.0' });
    autoUpdater.emit('update-available', { version: '2.0.0' });
    assert.equal(notifications.length, 1);
    assert.equal(notifications[0].title, 'Oya Browser 2.0.0 is available');
  });

  it('installs only a downloaded update, after the handler returns and the jar is on disk', async () => {
    const order: string[] = [];
    await setUp(true, async () => order.push('flush')).start();
    assert.equal(handlers['install-update'](), false);
    autoUpdater.emit('update-downloaded', { version: '2.0.0' });
    autoUpdater.quitAndInstall = () => order.push('install');
    assert.equal(handlers['install-update'](), true);
    assert.deepEqual(order, []);
    mock.timers.tick(0);
    await new Promise((resolve) => process.nextTick(resolve));
    await Promise.resolve();
    assert.deepEqual(order, ['flush', 'install']);
  });

  it('shows a failed manual check as an error', async () => {
    await setUp(true).start();
    autoUpdater.checkForUpdates = async () => {
      throw new Error('404');
    };
    const state = await handlers['check-for-updates']();
    assert.deepEqual([state.state, state.message], ['error', '404']);
  });
});
