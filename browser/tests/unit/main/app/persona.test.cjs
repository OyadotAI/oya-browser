/**
 * Unit tests for Persona: the partition per persona, the fingerprint summary,
 * the switch that drops queued cookies before closing the old jar's tabs, and
 * the login-state transport kept per persona.
 */
const { describe, it, beforeEach } = require('node:test');
const assert = require('node:assert/strict');
const { Persona, fingerprintSummary } = require('../../../../main/app/persona.cjs');
const { mainCtx } = require('../../support/main-ctx.cjs');

const PROFILE = {
  id: 'p1',
  navigator: { platform: 'MacIntel', hardwareConcurrency: 8, deviceMemory: 16 },
  screen: { width: 1512, height: 982, devicePixelRatio: 2 },
  webgl: { unmaskedRenderer: 'Apple M2' },
  timezone: 'Europe/Berlin',
  locale: 'de-DE',
  fonts: { available: ['a', 'b'] },
  canvas: { noiseSeed: 0.1234567 },
  audio: { noiseSeed: 0.5 },
};

describe('Persona', () => {
  let ctx, order;
  beforeEach(() => {
    ctx = mainCtx({ persona: Persona });
    order = [];
    const tabs = [
      { id: 1, url: 'https://a.test/', view: { webContents: { isDestroyed: () => false } } },
      { id: 2, url: '', view: {} },
    ];
    ctx.tabs = {
      list: tabs,
      closeTab: (id, options) => (
        order.push(['close', id, options]),
        tabs.splice(
          tabs.findIndex((t) => t.id === id),
          1,
        )
      ),
      createTab: (url) => order.push(['open', url]),
    };
    ctx.cookies = {
      dropPendingCookieChanges: () => order.push('drop'),
      forgetPulls: () => order.push('forget'),
      startCookieChangeListener: () => order.push('listen'),
      applyCookieSync: async (c) => order.push(['cookies', c.length]),
    };
    ctx.persona.setupBrowserSession = async () => order.push('session');
  });

  it('keeps each persona in its own partition', () => {
    assert.equal(ctx.persona.partitionName(), 'persist:oya-browser');
    ctx.persona.active = PROFILE;
    assert.equal(ctx.persona.partitionName(), 'persist:oya-p1');
    assert.equal(ctx.persona.session().name, 'persist:oya-p1');
  });

  it('summarizes a fingerprint for the debug bar', () => {
    assert.deepEqual(fingerprintSummary(PROFILE), {
      id: 'p1',
      platform: 'MacIntel',
      hardwareConcurrency: 8,
      deviceMemory: 16,
      screen: '1512x982',
      dpr: 2,
      gpu: 'Apple M2',
      timezone: 'Europe/Berlin',
      locale: 'de-DE',
      fonts: 2,
      canvasNoise: '0.123457',
      audioNoise: '0.500000',
    });
    assert.equal(ctx.persona.summary(), null);
  });

  it('switching persona drops queued cookies, empties the old jar, and reopens its pages in the new one', async () => {
    await ctx.persona.applyServerFingerprint(PROFILE, [{}, {}]);
    assert.deepEqual(order, [
      'drop',
      ['close', 1, { keepOne: false }],
      ['close', 2, { keepOne: false }],
      'forget',
      'session',
      'listen',
      ['cookies', 2],
      ['open', 'https://a.test/'],
      ['open', 'about:blank'],
    ]);
    assert.equal(ctx.config.values.activeProfileId, 'p1');
    assert.equal(ctx.shell.sentOn('fingerprint-changed')[0].id, 'p1');
  });

  it('re-protects open tabs when the same persona is sent again', async () => {
    const protectedViews = [];
    ctx.protection.setupTabCDP = (view) => protectedViews.push(view);
    ctx.persona.active = PROFILE;
    ctx.tabs.list.pop();
    await ctx.persona.applyServerFingerprint(PROFILE);
    assert.equal(protectedViews.length, 1);
    assert.ok(!order.includes('drop'));
  });

  it('ignores a profile without an id', async () => {
    await ctx.persona.applyServerFingerprint({});
    assert.deepEqual(order, []);
  });

  it('keeps the login state for the same persona and forwards storage changes only when online', () => {
    ctx.persona.ensureLoginState({ origins: {}, fingerprint: { id: 'p1' } });
    const first = ctx.persona.loginState;
    ctx.persona.active = PROFILE;
    ctx.persona.ensureLoginState({ fingerprint: { id: 'p1' } });
    assert.equal(ctx.persona.loginState, first);
    first.onChange({ 'https://a.test': { k: 'v' } });
    ctx.socket.ready = false;
    first.onChange({});
    assert.deepEqual(ctx.socket.ofType('storage_changed'), [
      { type: 'storage_changed', origins: { 'https://a.test': { k: 'v' } } },
    ]);
  });
});
