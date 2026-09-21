/**
 * Unit tests for fingerprint.ts: persona seeds, the prefs a persona may ask
 * for, and the memoised fingerprint that must never change for a persona.
 */
import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  PREF_OPTIONS,
  PLATFORMS,
  prefsError,
  defaultPersonaSeed,
  newPersonaSeed,
  previewProfile,
  getFingerprintForPersona,
  getFingerprintForKey,
} from '../../../../src/modules/personas/fingerprint.ts';
import { generateProfile } from '../../../../src/modules/personas/profile.ts';
import { seedFromString } from '../../../../src/modules/personas/prng.ts';

describe('prefsError', () => {
  it('accepts no prefs at all', () => {
    assert.equal(prefsError(null), null);
    assert.equal(prefsError('Win32'), null);
  });

  it('accepts offered choices, and "auto" or empty values as no choice', () => {
    assert.equal(prefsError({ platform: 'MacIntel', timezone: 'Europe/Berlin', locale: 'de-DE' }), null);
    assert.equal(prefsError({ platform: 'auto', timezone: '', locale: undefined }), null);
  });

  it('names the first choice that is not offered and where to find the list', () => {
    assert.equal(
      prefsError({ platform: 'Amiga' }),
      'platform "Amiga" is not offered for personas; GET /api/personas/options lists the choices',
    );
    assert.match(prefsError({ timezone: 'Mars/Olympus' }), /^timezone "Mars\/Olympus"/);
    assert.match(prefsError({ locale: 'xx-XX' }), /^locale "xx-XX"/);
  });
});

describe('PREF_OPTIONS', () => {
  it('offers every timezone and locale choice on every platform', () => {
    assert.deepEqual(PREF_OPTIONS.platforms, PLATFORMS);
    for (const p of PLATFORMS) {
      assert.ok(PREF_OPTIONS.timezones[p].includes('Europe/Berlin'));
      assert.ok(PREF_OPTIONS.locales[p].includes('en-US'));
    }
  });
});

describe('persona seeds', () => {
  it("derives a key's default persona id and seed from the key alone", () => {
    assert.deepEqual(defaultPersonaSeed('key-a'), { id: 'apikey-f10f781241e2', seed: 101943411 });
  });

  it('gives a new persona a random id and a seed hashed from that id', () => {
    const a = newPersonaSeed();
    const b = newPersonaSeed();
    assert.match(a.id, /^p-[0-9a-f]{16}$/);
    assert.notEqual(a.id, b.id);
    assert.equal(a.seed, seedFromString(a.id));
  });
});

describe('getFingerprintForPersona', () => {
  beforeEach(() => mock.method(console, 'log', () => {}));

  it('returns null for an identity without an id', () => {
    assert.equal(getFingerprintForPersona(null), null);
    assert.equal(getFingerprintForPersona({ seed: 1 }), null);
  });

  it('returns the very same profile for a persona on every call', () => {
    const first = getFingerprintForPersona({ id: 'fp-same', seed: 99 });
    assert.equal(getFingerprintForPersona({ id: 'fp-same', seed: 99 }), first);
  });

  it('is the profile the seed generates', () => {
    assert.deepEqual(getFingerprintForPersona({ id: 'fp-gen', seed: 5 }), generateProfile({ id: 'fp-gen', seed: 5 }));
  });

  it('keeps the first profile for an id even if asked with another seed', () => {
    const first = getFingerprintForPersona({ id: 'fp-stable', seed: 1 });
    assert.equal(getFingerprintForPersona({ id: 'fp-stable', seed: 2 }), first);
  });
});

describe('getFingerprintForKey', () => {
  beforeEach(() => mock.method(console, 'log', () => {}));

  it('returns null without a key', () => {
    assert.equal(getFingerprintForKey(''), null);
  });

  it("is the key's default persona fingerprint, as it was before personas existed", () => {
    const expected = generateProfile(defaultPersonaSeed('key-legacy'));
    assert.deepEqual(getFingerprintForKey('key-legacy'), expected);
    assert.equal(getFingerprintForKey('key-legacy').id, defaultPersonaSeed('key-legacy').id);
  });
});

describe('previewProfile', () => {
  it('generates without caching, so previews do not accumulate', () => {
    mock.method(console, 'log', () => {});
    const preview = previewProfile({ id: 'fp-preview', seed: 3 });
    assert.notEqual(getFingerprintForPersona({ id: 'fp-preview', seed: 3 }), preview);
    assert.deepEqual(getFingerprintForPersona({ id: 'fp-preview', seed: 3 }), preview);
  });
});
