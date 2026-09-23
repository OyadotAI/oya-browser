/**
 * Unit tests for start-up order and quitting: a recording is saved before the
 * app quits, a validation running at quit is marked interrupted, and the
 * cookie jar is on disk before the app goes.
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
    assert.equal(quit(), true, 'the second quit waits for the jar');
    assert.equal(quit(), false, 'the third goes through');
  });

  it('holds the quit until the cookies and storage are on disk, then quits', async () => {
    const steps = [];
    ctx.persona.flushJar = async () => steps.push('jar');
    ctx.electron.app.quit = () => steps.push('quit');
    assert.equal(quit(), true);
    await new Promise((r) => setImmediate(r));
    assert.deepEqual(steps, ['jar', 'quit']);
    assert.equal(quit(), false, 'the quit after the flush goes through');
  });

  it('quits even when the jar cannot be written', async () => {
    ctx.persona.flushJar = async () => {
      throw new Error('disk full');
    };
    assert.equal(quit(), true);
    await new Promise((r) => setImmediate(r));
    assert.equal(ctx.electron.app.quitted, true);
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
    quit();
    assert.equal(received[0].status, 'interrupted');
    assert.equal(ctx.workspace.session.disposed, true);
    assert.equal(flushed, true);
  });

  it('sends the cookie changes still queued, then disconnects and quits, when the last window closes', () => {
    let socketUpAtFlush = null;
    ctx.cookies.flushCookieChanges = () => (socketUpAtFlush = !ctx.socket.disconnects);
    ctx.electron.app.emit('window-all-closed');
    assert.equal(socketUpAtFlush, true, 'a login made a moment ago goes out while the socket is still up');
    assert.equal(ctx.socket.disconnects, 1);
    assert.equal(ctx.electron.app.quitted, true);
  });
});

/** A context whose boot steps record themselves in `order`, with a scratch userData directory. */
function bootCtx(order) {
  const ctx = mainCtx();
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-boot-'));
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
  ctx.tabs = { enterBrowsingMode: (url) => order.push(['browse', url]) };
  ctx.startAutoUpdate = () => order.push('update');
  ctx.deepLinks = { drain: async () => order.push('links') };
  return { ctx, dir, done: () => fs.rmSync(dir, { recursive: true, force: true }) };
}

describe('bootBrowser', () => {
  it('loads settings and persona, then the workspace, the window, cookie sync, the connection and updates, in order', async () => {
    const order = [];
    const { ctx, dir, done } = bootCtx(order);
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
    done();
  });

  it('opens straight to browsing on a blank tab when this desktop has signed in before', async () => {
    const order = [];
    const { ctx, done } = bootCtx(order);
    ctx.persona.loadActive = () => (ctx.persona.active = { id: 'p1' });
    await bootBrowser(ctx);
    assert.deepEqual(order.slice(order.indexOf('connect'), -2), ['connect', ['browse', 'about:blank']]);
    done();
  });

  it('shows the welcome screen without a saved key, even with a saved persona', async () => {
    const order = [];
    const { ctx, done } = bootCtx(order);
    ctx.persona.loadActive = () => (ctx.persona.active = { id: 'p1' });
    ctx.config.load = () => (ctx.config.values = { apiKey: '' });
    await bootBrowser(ctx);
    assert.ok(!order.some((step) => step[0] === 'browse'));
    done();
  });
});
