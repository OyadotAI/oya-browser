/**
 * Unit tests for Protection: the stealth-only fallback, one-time tab setup,
 * the dialog watcher next to Page.enable, popups, and loud failures.
 */
const { describe, it, beforeEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { generateProfile } = require('../../../../anonymity/fingerprint');
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

  it("presents Chrome's identity and injects the stealth script before a persona arrives", async () => {
    const dbg = new FakeDebugger();
    await ctx.protection.applyPersona(dbg, () => {});
    assert.deepEqual(
      dbg.sent.map((c) => c.method),
      ['Emulation.setUserAgentOverride', 'Page.addScriptToEvaluateOnNewDocument', 'Target.setAutoAttach'],
    );
    const brands = dbg.sent[0].params.userAgentMetadata.brands.map((b) => b.brand);
    assert.ok(brands.includes('Google Chrome'), 'navigator.userAgentData names Chrome, as the headers do');
    assert.equal(typeof dbg.sent[1].params.source, 'string');
  });

  it('attaches cross-site iframes without a persona, never pausing them, so a recording can reach them', async () => {
    const dbg = new FakeDebugger();
    await ctx.protection.applyPersona(dbg, () => {});
    const attach = dbg.sent.find((c) => c.method === 'Target.setAutoAttach').params;
    assert.deepEqual(
      [attach.autoAttach, attach.waitForDebuggerOnStart, attach.flatten, attach.filter[0].type],
      [true, false, true, 'iframe'],
    );
  });

  it("keeps this machine's timezone for a persona that leaves by this machine's own connection", async () => {
    const here = Intl.DateTimeFormat().resolvedOptions().timeZone;
    const elsewhere = here === 'Asia/Tokyo' ? 'Europe/Paris' : 'Asia/Tokyo';
    ctx.persona.active = { ...generateProfile({ seed: 'tz', platform: 'MacIntel' }), timezone: elsewhere, proxy: null };
    const dbg = new FakeDebugger();
    await ctx.protection.applyPersona(dbg, () => {});
    const zone = dbg.sent.find((c) => c.method === 'Emulation.setTimezoneOverride').params.timezoneId;
    assert.equal(zone, here, 'a persona zone over a home IP reads as "timezone spoofed"');
    const injected = dbg.sent.find((c) => c.method === 'Page.addScriptToEvaluateOnNewDocument').params.source;
    assert.ok(!injected.includes(elsewhere), 'and the injection does not patch Intl to the persona zone either');
  });

  /** The canvas noise seed inside the script injected for the active persona. */
  const injectedCanvasSeed = async () => {
    const dbg = new FakeDebugger();
    await ctx.protection.applyPersona(dbg, () => {});
    const source = dbg.sent.find((c) => c.method === 'Page.addScriptToEvaluateOnNewDocument').params.source;
    return JSON.parse(source.match(/"canvas":(\{[^}]*\})/)[1]).noiseSeed;
  };

  it("paints canvas as this computer really does on a person's own machine: noise reads as masking", async () => {
    ctx.persona.active = generateProfile({ seed: 'canvas', platform: 'MacIntel' });
    assert.equal(typeof ctx.persona.active.canvas.noiseSeed, 'number');
    assert.equal(await injectedCanvasSeed(), null);
  });

  it("leaves the screen this computer's own: a size spoofed in JavaScript alone disagrees with the real window", async () => {
    ctx.persona.active = generateProfile({ seed: 'screen', platform: 'MacIntel' });
    const dbg = new FakeDebugger();
    await ctx.protection.applyPersona(dbg, () => {});
    const source = dbg.sent.find((c) => c.method === 'Page.addScriptToEvaluateOnNewDocument').params.source;
    assert.match(source, /"screen":null/);
  });

  it('keeps the canvas noise in a cloud image, where every browser would otherwise paint the same known hash', async () => {
    ctx.persona.active = generateProfile({ seed: 'canvas', platform: 'MacIntel' });
    ctx.config.values.provider = 'oya-cloud';
    assert.equal(await injectedCanvasSeed(), ctx.persona.active.canvas.noiseSeed);
  });

  it("presents the persona's timezone when it leaves through the persona's proxy", async () => {
    const proxied = { ...generateProfile({ seed: 'tz', platform: 'MacIntel' }), timezone: 'Asia/Tokyo' };
    ctx.persona.active = { ...proxied, proxy: { host: 'gate.test', port: 8080 } };
    const dbg = new FakeDebugger();
    await ctx.protection.applyPersona(dbg, () => {});
    const zone = dbg.sent.find((c) => c.method === 'Emulation.setTimezoneOverride').params.timezoneId;
    assert.equal(zone, 'Asia/Tokyo');
  });

  it("overrides the page's user agent data with the persona's, not only the session's string", async () => {
    ctx.persona.active = generateProfile({ seed: 'ua', platform: 'Win32' });
    const dbg = new FakeDebugger();
    await ctx.protection.applyPersona(dbg, () => {});
    const override = dbg.sent.find((c) => c.method === 'Emulation.setUserAgentOverride').params;
    assert.equal(override.platform, 'Win32');
    assert.equal(override.userAgentMetadata.platform, 'Windows');
    assert.match(override.userAgent, /Windows NT 10\.0/);
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
    assert.match(errors.mock.calls[0].arguments[0], /debugger attach failed, this attempt did not protect the tab/);
  });

  it('answers false when the injection is refused, and true for a tab that held', async () => {
    mock.method(console, 'error', () => {});
    const refused = new FakeBrowserView();
    refused.webContents.debugger.responses['Page.addScriptToEvaluateOnNewDocument'] = new Error('gone');
    assert.equal(await ctx.protection.setupTabCDP(refused), false);
    assert.equal(await ctx.protection.setupTabCDP(new FakeBrowserView()), true);
  });

  it('a hung first attempt that wakes after the retry sends nothing to the new session', async () => {
    const view = new FakeBrowserView();
    const dbg = view.webContents.debugger;
    let wake;
    const hung = new Promise((resolve) => (wake = resolve));
    let first = true;
    dbg.responses['Emulation.setUserAgentOverride'] = () => (first ? ((first = false), hung) : {});
    const stale = ctx.protection.setupTabCDP(view);
    ctx.protection.resetTabCDP(view);
    assert.equal(await ctx.protection.setupTabCDP(view), true);
    wake({});
    assert.equal(await stale, false);
    const injections = dbg.methods().filter((m) => m === 'Page.addScriptToEvaluateOnNewDocument');
    assert.equal(injections.length, 1, 'the page was injected twice');
  });

  it('a second caller while setup runs gets that attempt\u2019s own answer, not an early true', async () => {
    mock.method(console, 'error', () => {});
    const view = new FakeBrowserView();
    let refuse;
    view.webContents.debugger.responses['Page.addScriptToEvaluateOnNewDocument'] = () =>
      new Promise((resolve) => (refuse = () => resolve(new Error('refused'))));
    const first = ctx.protection.setupTabCDP(view);
    const second = ctx.protection.setupTabCDP(view);
    await new Promise((r) => setImmediate(r));
    refuse();
    assert.deepEqual(await Promise.all([first, second]), [false, false]);
  });

  it('keeps the dialog watcher and every other listener when a tab is reset for a retry', async () => {
    const view = new FakeBrowserView();
    await ctx.protection.setupTabCDP(view);
    const dbg = view.webContents.debugger;
    const before = dbg.listenerCount('message');
    ctx.protection.resetTabCDP(view);
    assert.equal(dbg.listenerCount('message'), before);
    assert.deepEqual([view.oyaConfigured, dbg.isAttached()], [false, false]);
  });

  it('forgets the recording channel a reset tab may have armed', () => {
    const forgot = [];
    ctx.recorder = { channels: { forget: (v) => forgot.push(v) } };
    const view = new FakeBrowserView();
    ctx.protection.resetTabCDP(view);
    assert.deepEqual(forgot, [view]);
  });

  it('an isolated-world failure never says the tab is NOT protected, and a closed tab says nothing', async () => {
    const errors = mock.method(console, 'error', () => {});
    mock.method(ctx.world, 'ensureWorld', async () => Promise.reject(new Error('target closed')));
    const view = new FakeBrowserView();
    await ctx.protection.setupTabCDP(view);
    view.webContents.emit('did-finish-load');
    await new Promise((r) => setImmediate(r));
    assert.match(errors.mock.calls[0].arguments[0], /isolated world not rebuilt/);
    assert.doesNotMatch(errors.mock.calls[0].arguments[0], /NOT protected/);
    view.webContents.destroyed = true;
    view.webContents.emit('did-finish-load');
    await new Promise((r) => setImmediate(r));
    assert.equal(errors.mock.callCount(), 1);
  });

  it('rebuilds the world once per load, however many attempts setup took', async () => {
    const view = new FakeBrowserView();
    const ensure = mock.method(ctx.world, 'ensureWorld', async () => 1);
    await ctx.protection.setupTabCDP(view);
    ctx.protection.resetTabCDP(view);
    await ctx.protection.setupTabCDP(view);
    view.webContents.emit('did-finish-load');
    assert.equal(ensure.mock.callCount(), 1);
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
