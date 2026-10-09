/**
 * Unit tests for Persona: the partition per persona, the fingerprint summary,
 * the switch that drops queued cookies before closing the old jar's tabs, and
 * the login-state transport kept per persona.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { Persona, fingerprintSummary } from '../../../../src/main/app/persona.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

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
      { id: 3, url: '', home: true, view: {} },
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
      applyCookieSync: async (c, clock) => order.push(['cookies', c.length, clock.now]),
    };
    ctx.persona.setupBrowserSession = async () => order.push('session');
  });

  it('switching persona closes each tab once when the multi-window router returns snapshots', async () => {
    const live = ctx.tabs.list;
    const close = ctx.tabs.closeTab;
    Object.defineProperty(ctx.tabs, 'list', { get: () => [...live] });
    let attempts = 0;
    ctx.tabs.closeTab = (id, options) => {
      assert.ok(++attempts <= 3, 'a closed tab must never be retried from a stale snapshot');
      return close(id, options);
    };
    await ctx.persona.applyServerFingerprint(PROFILE, [], 9000);
    assert.deepEqual(
      order
        .filter(Array.isArray)
        .filter((item) => item[0] === 'close')
        .map((item) => item[1]),
      [1, 2, 3],
    );
    assert.equal(live.length, 0);
    assert.deepEqual(
      order
        .filter(Array.isArray)
        .filter((item) => item[0] === 'open')
        .map((item) => item[1]),
      ['https://a.test/', 'about:blank', 'oya:home'],
    );
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

  it('switching persona drops queued cookies, empties the old jar, and reopens its pages, start page included, in the new one', async () => {
    await ctx.persona.applyServerFingerprint(PROFILE, [{}, {}], 9000);
    assert.deepEqual(order, [
      'drop',
      ['close', 1, { keepOne: false }],
      ['close', 2, { keepOne: false }],
      ['close', 3, { keepOne: false }],
      'forget',
      'session',
      'listen',
      ['cookies', 2, 9000],
      ['open', 'https://a.test/'],
      ['open', 'about:blank'],
      ['open', 'oya:home'],
    ]);
    assert.equal(ctx.config.values.activeProfileId, 'p1');
    assert.equal(ctx.shell.sentOn('fingerprint-changed')[0].id, 'p1');
  });

  it("runs as a persona mirrored from the person's real browser, whose device has no font list and no noise", async () => {
    const mirrored = {
      ...PROFILE,
      id: 'm-1',
      fonts: undefined,
      canvas: { noiseSeed: null },
      audio: { noiseSeed: null },
    };
    await ctx.persona.applyServerFingerprint(mirrored, []);
    const shown = ctx.shell.sentOn('fingerprint-changed').at(-1);
    assert.deepEqual([shown.id, shown.fonts, shown.canvasNoise, shown.audioNoise], ['m-1', 0, 'real', 'real']);
    assert.equal(ctx.persona.summary().id, 'm-1');
  });

  it('re-protects open tabs when the same persona is sent again', async () => {
    const protectedViews = [];
    ctx.protection.setupTabCDP = (view) => protectedViews.push(view);
    ctx.persona.active = PROFILE;
    ctx.tabs.list.splice(1);
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
