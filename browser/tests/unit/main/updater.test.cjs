/**
 * Unit tests for main/updater.cjs: the toolbar's update state, the IPC
 * channels, and why dev runs and cloud browsers never update. Electron and
 * electron-updater are faked.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { installElectron, freshRequire } = require('../support/fakes.cjs');
const { UPDATE_FIRST_CHECK_MS, UPDATE_CHECK_INTERVAL_MS } = require('../../../main/constants.cjs');

/** Puts a fake electron-updater in the require cache; returns it and a restore function. */
function installAutoUpdater() {
  const autoUpdater = new EventEmitter();
  autoUpdater.checks = 0;
  autoUpdater.checkForUpdates = async () => {
    autoUpdater.checks++;
  };
  autoUpdater.quitAndInstall = () => (autoUpdater.installed = true);
  const id = require.resolve('electron-updater');
  const previous = require.cache[id];
  require.cache[id] = { id, filename: id, loaded: true, exports: { autoUpdater } };
  return { autoUpdater, restore: () => (previous ? (require.cache[id] = previous) : delete require.cache[id]) };
}

describe('createUpdater', () => {
  let app;
  let notifications;
  let restoreElectron;
  let updater;
  let handlers;
  let statuses;

  /** A packaged (or dev) app with the updater created and its IPC captured. */
  function setUp(isPackaged, beforeInstall) {
    app = { isPackaged, getVersion: () => '1.0.0' };
    notifications = [];
    /** Records each OS notification shown. */
    class Notification {
      /** Keeps the options. */
      constructor(options) {
        this.options = options;
      }
      /** Records it. */
      show() {
        notifications.push(this.options);
      }
    }
    Notification.isSupported = () => true;
    restoreElectron = installElectron({ app, Notification });
    handlers = {};
    statuses = [];
    const { createUpdater } = freshRequire('main/updater.cjs');
    return createUpdater({
      handle: (channel, fn) => (handlers[channel] = fn),
      sendToRenderer: (channel, state) => statuses.push([channel, state]),
      beforeInstall,
    });
  }

  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout', 'setInterval', 'setImmediate'] });
    mock.method(console, 'log', () => {});
    updater = installAutoUpdater();
  });
  afterEach(() => {
    mock.timers.reset();
    mock.restoreAll();
    restoreElectron();
    updater.restore();
  });

  it('registers the update IPC channels', () => {
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
    setUp(false).startAutoUpdate();
    assert.deepEqual(statuses, [['update-status', { state: 'unsupported', version: null, current: '1.0.0' }]]);
    assert.deepEqual(await handlers['check-for-updates'](), statuses[0][1]);
    assert.equal(updater.autoUpdater.checks, 0);
  });

  it('never updates a cloud browser, even when packaged', () => {
    process.env.OYA_DOCKER = 'true';
    try {
      setUp(true).startAutoUpdate();
    } finally {
      delete process.env.OYA_DOCKER;
    }
    assert.equal(statuses[0][1].state, 'unsupported');
  });

  it('checks after the first window settles and then on an interval', async () => {
    setUp(true).startAutoUpdate();
    assert.equal(updater.autoUpdater.checks, 0);
    mock.timers.tick(UPDATE_FIRST_CHECK_MS);
    assert.equal(updater.autoUpdater.checks, 1);
    mock.timers.tick(UPDATE_CHECK_INTERVAL_MS);
    assert.equal(updater.autoUpdater.checks, 2);
    assert.equal(updater.autoUpdater.autoDownload, true);
    assert.deepEqual(updater.autoUpdater.requestHeaders, { 'X-Oya-Version': '1.0.0' }, 'checks say which version asks');
    assert.equal(updater.autoUpdater.autoInstallOnAppQuit, true);
  });

  it('mirrors each updater event into the toolbar state', () => {
    setUp(true).startAutoUpdate();
    const { autoUpdater } = updater;
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

  it('announces a new version at the OS level once, not on every re-check', () => {
    setUp(true).startAutoUpdate();
    updater.autoUpdater.emit('update-available', { version: '2.0.0' });
    updater.autoUpdater.emit('update-available', { version: '2.0.0' });
    assert.equal(notifications.length, 1);
    assert.equal(notifications[0].title, 'Oya Browser 2.0.0 is available');
  });

  it('installs only a downloaded update, after the handler returns and the jar is on disk', async () => {
    const order = [];
    setUp(true, async () => order.push('flush')).startAutoUpdate();
    assert.equal(handlers['install-update'](), false);
    updater.autoUpdater.emit('update-downloaded', { version: '2.0.0' });
    updater.autoUpdater.quitAndInstall = () => order.push('install');
    assert.equal(handlers['install-update'](), true);
    assert.deepEqual(order, []);
    mock.timers.tick(0);
    await new Promise((resolve) => process.nextTick(resolve));
    await Promise.resolve();
    assert.deepEqual(order, ['flush', 'install']);
  });

  it('shows a failed manual check as an error', async () => {
    setUp(true).startAutoUpdate();
    updater.autoUpdater.checkForUpdates = async () => {
      throw new Error('404');
    };
    const state = await handlers['check-for-updates']();
    assert.deepEqual([state.state, state.message], ['error', '404']);
  });
});
