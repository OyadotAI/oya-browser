/**
 * Unit tests for start-up: settings and persona, then the workspace, the
 * window and the services, in order; and a signed-in launch that opens
 * straight on the Oya start page, which loads nothing from the web, while a
 * browser without a key or a persona waits for the server instead.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { Boot } from '../../../../src/main/app/boot.ts';
import { HOME_URL } from '../../../../src/main/tabs/constants.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

/** A context whose boot steps record themselves in `order`, with a scratch userData directory. */
function bootCtx(order: any[]) {
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
  ctx.updater = { start: () => order.push('update') };
  ctx.deepLinks = { drain: async () => order.push('links') };
  return { ctx, dir, done: () => fs.rmSync(dir, { recursive: true, force: true }) };
}

describe('Boot', () => {
  it('loads settings and persona, then the workspace, the window, cookie sync, the connection and updates, in order', async () => {
    const order = [];
    const { ctx, dir, done } = bootCtx(order);
    await new Boot(ctx).run();
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

  it('opens straight to browsing on the start page when this desktop has signed in before', async () => {
    const order = [];
    const { ctx, done } = bootCtx(order);
    ctx.persona.loadActive = () => (ctx.persona.active = { id: 'p1' });
    await new Boot(ctx).run();
    assert.deepEqual(order.slice(order.indexOf('connect'), -2), ['connect', ['browse', 'oya:home']]);
    done();
  });

  it('shows the welcome screen without a saved key, even with a saved persona', async () => {
    const order = [];
    const { ctx, done } = bootCtx(order);
    ctx.persona.loadActive = () => (ctx.persona.active = { id: 'p1' });
    ctx.config.load = () => (ctx.config.values = { apiKey: '' });
    await new Boot(ctx).run();
    assert.ok(!order.some((step) => step[0] === 'browse'));
    done();
  });
});

/** A context that records where browsing was entered, signed in unless told otherwise. */
function launch({ apiKey = 'k', persona = { id: 'p1' } as object | null } = {}) {
  const ctx = mainCtx();
  ctx.config.values = { apiKey };
  ctx.persona.active = persona;
  ctx.tabs = { enterBrowsingMode: (url) => (ctx.entered = url) };
  return ctx;
}

describe('Boot.resumeSignedIn', () => {
  it('opens browsing on the start page when this desktop has signed in before', () => {
    const ctx = launch();
    new Boot(ctx).resumeSignedIn();
    assert.equal(ctx.entered, HOME_URL);
  });

  it('waits for the server without a saved key or persona', () => {
    for (const ctx of [launch({ apiKey: '' }), launch({ persona: null })]) {
      new Boot(ctx).resumeSignedIn();
      assert.equal(ctx.entered, undefined);
    }
  });
});
