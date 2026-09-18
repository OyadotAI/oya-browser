/**
 * Unit tests for Protection: the stealth-only fallback, one-time tab setup,
 * the dialog watcher next to Page.enable, popups, and loud failures.
 */
const { describe, it, beforeEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { Protection } = require('../../../../main/tabs/protection.cjs');
const { mainCtx, FakeBrowserView } = require('../../support/main-ctx.cjs');
const { FakeDebugger } = require('../../support/fakes.cjs');

describe('Protection', () => {
  let ctx;
  beforeEach(() => {
    ctx = mainCtx({ protection: Protection });
    ctx.world = { ensureWorld: async () => 1 };
  });

  it('injects the stealth script alone before a persona arrives', async () => {
    const dbg = new FakeDebugger();
    await ctx.protection.applyPersona(dbg, () => {});
    assert.equal(dbg.sent[0].method, 'Page.addScriptToEvaluateOnNewDocument');
    assert.equal(typeof dbg.sent[0].params.source, 'string');
  });

  it('reports a failed stealth injection instead of throwing', async () => {
    const dbg = new FakeDebugger({ 'Page.addScriptToEvaluateOnNewDocument': new Error('gone') });
    const failures = [];
    await ctx.protection.applyPersona(dbg, (what, e) => failures.push([what, e.message]));
    assert.deepEqual(failures, [['stealth injection', 'gone']]);
  });

  it('sets a tab up once, with a dialog watcher and the page domain on', async () => {
    const view = new FakeBrowserView();
    await ctx.protection.setupTabCDP(view);
    await ctx.protection.setupTabCDP(view);
    const methods = view.webContents.debugger.methods();
    assert.equal(methods.filter((m) => m === 'Page.enable').length, 1);
    assert.equal(view.webContents.debugger.oyaDialogWatcher, true);
    assert.equal(view.oyaConfigured, true);
  });

  it('attaches the login-state transport when there is one', async () => {
    const attached = [];
    ctx.persona.loginState = { attach: async (send, on) => attached.push([typeof send, typeof on]) };
    await ctx.protection.setupTabCDP(new FakeBrowserView());
    assert.deepEqual(attached, [['function', 'function']]);
  });

  it('says loudly when a destroyed tab cannot be protected', async () => {
    const errors = mock.method(console, 'error', () => {});
    const view = new FakeBrowserView();
    view.webContents.destroyed = true;
    await ctx.protection.setupTabCDP(view);
    assert.match(errors.mock.calls[0].arguments[0], /debugger attach failed — this tab is NOT protected/);
  });

  it('rebuilds the isolated world on every load', async () => {
    const view = new FakeBrowserView();
    const ensure = mock.method(ctx.world, 'ensureWorld', async () => 1);
    await ctx.protection.setupTabCDP(view);
    view.webContents.emit('did-finish-load');
    assert.deepEqual(ensure.mock.calls[0].arguments, [view, { force: true }]);
  });

  it('protects a popup before its scripts run and disables it for agents', () => {
    const adopted = mock.method(ctx.shield, 'adoptPopup', () => {});
    const child = Object.assign(new EventEmitter(), { webContents: { debugger: new FakeDebugger() } });
    ctx.protection.protectPopup(child);
    const dbg = child.webContents.debugger;
    assert.equal(adopted.mock.callCount(), 1);
    assert.equal(dbg.attached, true);
    assert.ok(dbg.methods().includes('Page.enable'));
    assert.equal(dbg.oyaDialogWatcher, true);
  });

  it('logs a popup whose debugger cannot attach', () => {
    const errors = mock.method(console, 'error', () => {});
    const dbg = new FakeDebugger();
    dbg.attach = () => {
      throw new Error('busy');
    };
    ctx.protection.protectPopup({ webContents: { debugger: dbg } });
    assert.match(errors.mock.calls[0].arguments[0], /popup debugger attach failed/);
  });

  it('loads the analyzer into the active tab by default, and survives a missing world', async () => {
    const errors = mock.method(console, 'error', () => {});
    ctx.tabs = { getActiveView: () => 'view' };
    ctx.world.ensureWorld = async () => {
      throw new Error('no frame');
    };
    await ctx.protection.injectScripts();
    assert.match(errors.mock.calls[0].arguments[0], /isolated world unavailable/);
    ctx.tabs = { getActiveView: () => null };
    await ctx.protection.injectScripts();
  });
});
