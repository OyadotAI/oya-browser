/**
 * Unit tests for generateProfile: a device drawn from a persona's seed and
 * prefs, identical every time, with prefs that never shift the rest of the
 * seeded stream.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { generateProfile } from '../../../../src/modules/personas/profile.ts';
import { GPU_DB, PLATFORMS } from '../../../../src/modules/personas/devices.ts';
import { COLOR_DEPTH, TASKBAR_PX } from '../../../../src/modules/personas/constants.ts';

describe('generateProfile', () => {
  it('draws the same device from the same seed and prefs, every time', () => {
    const identity = { id: 'p-1', seed: 424242, prefs: { platform: 'MacIntel', checked: true } };
    assert.deepEqual(generateProfile(identity), generateProfile(structuredClone(identity)));
  });

  it('keeps giving seed 42 the device it has always had', () => {
    const p = generateProfile({ id: 'x', seed: 42 });
    assert.deepEqual(
      {
        platform: p.navigator.platform,
        gpu: p.webgl.unmaskedRenderer,
        screen: `${p.screen.width}x${p.screen.height}`,
        timezone: p.timezone,
        locale: p.locale,
        cores: p.navigator.hardwareConcurrency,
        memory: p.navigator.deviceMemory,
        canvas: p.canvas.noiseSeed,
      },
      {
        platform: 'Win32',
        gpu: 'NVIDIA GeForce RTX 3060/PCIe/SSE2',
        screen: '1366x768',
        timezone: 'America/Indianapolis',
        locale: 'en-GB',
        cores: 6,
        memory: 8,
        canvas: 0.0002566390485168991,
      },
    );
  });

  it('gives different seeds different devices', () => {
    const a = generateProfile({ id: 'a', seed: 1 });
    const b = generateProfile({ id: 'b', seed: 2 });
    assert.notDeepEqual({ ...a, id: null }, { ...b, id: null });
  });

  it('honours a platform pref and draws that platform’s hardware', () => {
    const p = generateProfile({ id: 'mac', seed: 42, prefs: { platform: 'MacIntel' } });
    assert.equal(p.navigator.platform, 'MacIntel');
    assert.ok(GPU_DB.MacIntel.includes(p.webgl));
  });

  it('ignores a platform it does not know and lets the seed decide', () => {
    const seeded = generateProfile({ id: 'x', seed: 42 });
    assert.deepEqual(generateProfile({ id: 'x', seed: 42, prefs: { platform: 'Amiga' } }), seeded);
  });

  it('draws the same noise seeds whether or not a platform was chosen', () => {
    const seeded = generateProfile({ id: 'x', seed: 42 });
    const chosen = generateProfile({ id: 'x', seed: 42, prefs: { platform: seeded.navigator.platform } });
    assert.deepEqual(chosen.canvas, seeded.canvas);
    assert.deepEqual(chosen.audio, seeded.audio);
    assert.deepEqual(chosen.rects, seeded.rects);
  });

  it('keeps the noise seeds when only the timezone or locale is chosen', () => {
    const seeded = generateProfile({ id: 'x', seed: 42 });
    const chosen = generateProfile({ id: 'x', seed: 42, prefs: { timezone: 'UTC', locale: 'de-DE', checked: true } });
    assert.deepEqual([chosen.canvas, chosen.webgl, chosen.screen], [seeded.canvas, seeded.webgl, seeded.screen]);
  });

  it('honours any offered timezone and locale for prefs checked under the current rule', () => {
    const p = generateProfile({
      id: 'x',
      seed: 42,
      prefs: { platform: 'Win32', timezone: 'Europe/Berlin', locale: 'de-DE', checked: true },
    });
    assert.equal(p.timezone, 'Europe/Berlin');
    assert.equal(p.locale, 'de-DE');
    assert.deepEqual(p.navigator.languages, ['de-DE', 'de']);
  });

  it('keeps an older persona on its seeded timezone when its pref is outside the platform table', () => {
    const seeded = generateProfile({ id: 'x', seed: 42, prefs: { platform: 'Win32' } });
    const old = generateProfile({ id: 'x', seed: 42, prefs: { platform: 'Win32', timezone: 'Europe/Berlin' } });
    assert.equal(old.timezone, seeded.timezone);
  });

  it('lets an older persona keep a pref its platform table has', () => {
    const p = generateProfile({ id: 'x', seed: 42, prefs: { platform: 'Win32', timezone: 'America/Chicago' } });
    assert.equal(p.timezone, 'America/Chicago');
  });

  it('marks Chrome-shaped WebGL only for prefs checked under the current rule', () => {
    assert.equal(generateProfile({ id: 'x', seed: 1, prefs: { checked: true } }).webglChrome, true);
    assert.equal(generateProfile({ id: 'x', seed: 1 }).webglChrome, false);
  });

  it('reports the screen minus the taskbar, at a fixed colour depth', () => {
    const { screen } = generateProfile({ id: 'x', seed: 42 });
    assert.equal(screen.availHeight, screen.height - TASKBAR_PX);
    assert.equal(screen.colorDepth, COLOR_DEPTH);
  });

  it('carries the persona id and proxy through', () => {
    const proxy = { host: 'proxy.example', port: 8080 };
    const p = generateProfile({ id: 'with-proxy', seed: 1, proxy });
    assert.equal(p.id, 'with-proxy');
    assert.deepEqual(p.proxy, proxy);
    assert.equal(generateProfile({ id: 'none', seed: 1 }).proxy, null);
  });

  it('only ever draws an offered platform', () => {
    for (let seed = 0; seed < 200; seed += 7) {
      assert.ok(PLATFORMS.includes(generateProfile({ id: 'x', seed: seed * 104729 }).navigator.platform));
    }
  });
});
