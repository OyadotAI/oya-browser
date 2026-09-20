/**
 * Unit tests for start-up order and quitting: a recording is saved before the
 * app quits, and a validation running at quit is marked interrupted.
 */
const { describe, it, beforeEach } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { Lifecycle, bootBrowser } = require('../../../../main/app/lifecycle.cjs');
const { mainCtx } = require('../../support/main-ctx.cjs');

describe('Lifecycle', () => {
  let ctx;
  beforeEach(() => {
    ctx = mainCtx();
    new Lifecycle(ctx).install();
  });

  /** Emits before-quit; returns whether it was held. */
  const quit = () => {
    const event = {
      held: false,
      preventDefault() {
        this.held = true;
      },
    };
    ctx.electron.app.emit('before-quit', event);
    return event.held;
  };

  it('holds the first quit until the recording is stopped, then quits', async () => {
    let stopped = false;
    ctx.recorder = {
      recording: true,
      queueRecording: (work) => Promise.resolve(work()),
      stopRecording: async () => (stopped = true),
    };
    assert.equal(quit(), true);
    await new Promise((r) => setImmediate(r));
    assert.equal(stopped, true);
    assert.equal(ctx.electron.app.quitted, true);
    assert.equal(quit(), false, 'the second quit goes through');
  });

  it('marks a running validation interrupted and flushes the panel', () => {
    let flushed = false;
    const received = [];
    ctx.workspace = {
      busy: () => true,
      receive: (m) => received.push(m),
      session: {
        dispose() {
          this.disposed = true;
        },
      },
    };
    ctx.layout.flush = () => (flushed = true);
    assert.equal(quit(), false);
    assert.equal(received[0].status, 'interrupted');
    assert.equal(ctx.workspace.session.disposed, true);
    assert.equal(flushed, true);
  });

  it('disconnects and quits when the last window closes', () => {
    ctx.electron.app.emit('window-all-closed');
    assert.equal(ctx.socket.disconnects, 1);
    assert.equal(ctx.electron.app.quitted, true);
  });
});

describe('bootBrowser', () => {
  it('loads settings and persona, then the workspace, the window, cookie sync, the connection and updates, in order', async () => {
    const ctx = mainCtx();
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-boot-'));
    const order = [];
    Object.assign(ctx.electron.app, { getPath: () => dir, setAsDefaultProtocolClient: () => order.push('protocol') });
    ctx.electron.Menu.setApplicationMenu = () => order.push('menu');
    ctx.electron.safeStorage = {
      isEncryptionAvailable: () => true,
      encryptString: (v) => Buffer.from(v),
      decryptString: (b) => b.toString(),
    };
    ctx.config.load = () => order.push('config');
    ctx.config.values = { apiKey: 'k' };
    ctx.persona.loadActive = (userData) => order.push(['persona', userData]);
    ctx.persona.setupBrowserSession = async () => order.push('session');
    ctx.recorder.adopt = () => order.push('adopt');
    ctx.shell.create = () => order.push('window');
    ctx.cookies.startCookieChangeListener = () => order.push('cookies');
    ctx.socket.connect = () => order.push('connect');
    ctx.startAutoUpdate = () => order.push('update');
    ctx.deepLinks = { drain: async () => order.push('links') };
    await bootBrowser(ctx);
    assert.deepEqual(order, [
      'menu',
      'config',
      'protocol',
      ['persona', dir],
      'session',
      'adopt',
      'window',
      'cookies',
      'connect',
      'update',
      'links',
    ]);
    assert.ok(ctx.workspace, 'the workspace exists before the window');
    fs.rmSync(dir, { recursive: true, force: true });
  });
});
