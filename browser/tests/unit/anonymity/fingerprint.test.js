/**
 * Unit tests for the GPU a persona reports (anonymity/fingerprint.js): which of
 * a profile's WebGL strings answer the UNMASKED_* queries.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { unmaskedWebgl, buildFingerprintBody, generateProfile } = require('../../../anonymity/fingerprint');

const ANGLE = 'ANGLE (Apple, ANGLE Metal Renderer: Apple M4, Unspecified Version)';

describe('unmaskedWebgl', () => {
  it("answers with Chrome's ANGLE strings for a persona that carries them", () => {
    const webgl = {
      vendor: 'Google Inc. (Apple)',
      renderer: ANGLE,
      unmaskedVendor: 'Apple',
      unmaskedRenderer: 'Apple M4',
    };
    assert.deepEqual(unmaskedWebgl({ webglChrome: true, webgl }), { vendor: 'Google Inc. (Apple)', renderer: ANGLE });
  });

  it('answers with the bare driver strings for an older persona, whose GPU must not change', () => {
    const webgl = {
      vendor: 'Google Inc. (Apple)',
      renderer: ANGLE,
      unmaskedVendor: 'Apple',
      unmaskedRenderer: 'Apple M1',
    };
    assert.deepEqual(unmaskedWebgl({ webgl }), { vendor: 'Apple', renderer: 'Apple M1' });
  });

  it('never answers "WebKit WebGL": a device mirrored before the fields were fixed keeps the real GPU in the unmasked pair', () => {
    const webgl = {
      vendor: 'WebKit',
      renderer: 'WebKit WebGL',
      unmaskedVendor: 'Google Inc. (Apple)',
      unmaskedRenderer: ANGLE,
    };
    assert.deepEqual(unmaskedWebgl({ webglChrome: true, webgl }), { vendor: 'Google Inc. (Apple)', renderer: ANGLE });
  });
});

describe('buildFingerprintBody', () => {
  it('leaves the screen alone for a profile that has none to present', () => {
    const profile = { ...generateProfile({ seed: 's', platform: 'MacIntel' }), screen: null };
    assert.doesNotThrow(
      () => new Function('"use strict";' + 'return 1;' + '/*' + buildFingerprintBody(profile).length + '*/'),
    );
    assert.match(buildFingerprintBody(profile), /if \(__fp\.screen\)/);
  });
});
