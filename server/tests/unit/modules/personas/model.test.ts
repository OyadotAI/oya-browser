/**
 * Unit tests for the persona model: normalising a persona from any source,
 * the prefs marker, and the public description of a fingerprint.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  shape,
  markChecked,
  publicPrefs,
  cleanPrefs,
  describeProfile,
  DEFAULT_MAX_CONCURRENT,
} from '../../../../src/modules/personas/model.ts';
import { generateProfile } from '../../../../src/modules/personas/profile.ts';
import { MAX_PREF_CHARS } from '../../../../src/modules/personas/constants.ts';

describe('shape', () => {
  it('keeps the persona fields and drops anything else', () => {
    const p = shape({ id: 'p', owner: 'o', name: 'n', seed: 1, createdAt: 't', secret: 'x' });
    assert.equal('secret' in p, false);
    assert.deepEqual(Object.keys(p).sort(), [
      'createdAt',
      'id',
      'isDefault',
      'lastUsedAt',
      'maxConcurrent',
      'name',
      'owner',
      'prefs',
      'proxy',
      'seed',
    ]);
  });

  it('reads a stored null cap as uncapped, and a missing one as the default', () => {
    assert.equal(shape({ maxConcurrent: null }).maxConcurrent, Infinity);
    assert.equal(shape({}).maxConcurrent, DEFAULT_MAX_CONCURRENT);
    assert.equal(shape({ maxConcurrent: 5 }).maxConcurrent, 5);
  });

  it('copies prefs rather than sharing them, and drops prefs that are not an object', () => {
    const prefs = { platform: 'Win32' };
    const p = shape({ prefs });
    assert.deepEqual(p.prefs, prefs);
    assert.notEqual(p.prefs, prefs);
    assert.equal(shape({ prefs: 'Win32' }).prefs, null);
  });

  it('defaults the proxy, default flag and last use', () => {
    const p = shape({});
    assert.deepEqual([p.proxy, p.isDefault, p.lastUsedAt], [null, false, null]);
  });
});

describe('markChecked', () => {
  it('marks prefs validated under the current rule, even when there were none', () => {
    assert.deepEqual(markChecked({ platform: 'Win32' }, true), { platform: 'Win32', checked: true });
    assert.deepEqual(markChecked(null, true), { checked: true });
  });

  it('leaves prefs of an older rule as they were', () => {
    const prefs = { platform: 'Win32' };
    assert.equal(markChecked(prefs, false), prefs);
    assert.equal(markChecked(null, false), null);
  });
});

describe('publicPrefs', () => {
  it('hides the internal marker', () => {
    assert.deepEqual(publicPrefs({ platform: 'Win32', checked: true }), { platform: 'Win32' });
  });

  it('is null when only the marker, or nothing, is left', () => {
    assert.equal(publicPrefs({ checked: true }), null);
    assert.equal(publicPrefs(null), null);
  });
});

describe('cleanPrefs', () => {
  it('keeps only the three device choices, as strings', () => {
    assert.deepEqual(cleanPrefs({ platform: 'Win32', timezone: 'UTC', locale: 'en-US', seed: 1, checked: true }), {
      platform: 'Win32',
      timezone: 'UTC',
      locale: 'en-US',
    });
    assert.deepEqual(cleanPrefs({ platform: 7, timezone: 'UTC' }), { timezone: 'UTC' });
  });

  it('treats "auto" and empty strings as no choice', () => {
    assert.equal(cleanPrefs({ platform: 'auto', timezone: '' }), null);
  });

  it('is null for anything that is not an object', () => {
    assert.equal(cleanPrefs(null), null);
    assert.equal(cleanPrefs('Win32'), null);
  });

  it('cuts a choice to the longest kept length', () => {
    assert.equal(cleanPrefs({ locale: 'x'.repeat(MAX_PREF_CHARS + 10) }).locale.length, MAX_PREF_CHARS);
  });
});

describe('describeProfile', () => {
  it('describes the device without its seeds', () => {
    const fp = generateProfile({ id: 'x', seed: 42 });
    assert.deepEqual(describeProfile(fp), {
      platform: 'Win32',
      timezone: 'America/Indianapolis',
      locale: 'en-GB',
      screen: '1366x768',
      webgl: 'NVIDIA GeForce RTX 3060/PCIe/SSE2',
      hardwareConcurrency: 6,
      deviceMemory: 8,
      canvasSeed: fp.canvas.noiseSeed,
    });
  });
});
