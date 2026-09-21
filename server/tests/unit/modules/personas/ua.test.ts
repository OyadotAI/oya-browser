/**
 * Unit tests for the user agent and client hints a persona presents: the OS
 * token must match the fingerprint's platform, and client hints are never
 * left blank.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { majorFrom, userAgentFor, metadataFor } from '../../../../src/modules/personas/ua.ts';

const HEADLESS =
  'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/131.0.6778.0 Safari/537.36';
/** A profile on this navigator.platform, optionally with a WebGL renderer. */
const on = (platform: string, renderer = '') => ({ navigator: { platform }, webgl: { renderer } });

describe('majorFrom', () => {
  it("reads Chrome's major version, or '' when there is none", () => {
    assert.equal(majorFrom(HEADLESS), '131');
    assert.equal(majorFrom('curl/8.0'), '');
    assert.equal(majorFrom(undefined), '');
  });
});

describe('userAgentFor', () => {
  it("presents the persona's OS with the browser's real Chrome major", () => {
    assert.equal(
      userAgentFor(on('Win32'), HEADLESS),
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
    );
    assert.match(userAgentFor(on('MacIntel'), HEADLESS), /\(Macintosh; Intel Mac OS X 10_15_7\).*Chrome\/131\.0\.0\.0/);
  });

  it('never says HeadlessChrome', () => {
    assert.doesNotMatch(userAgentFor(on('Linux x86_64'), HEADLESS), /Headless/);
  });

  it('maps an unknown Linux flavour to x86_64 and anything else to Windows', () => {
    assert.match(userAgentFor(on('Linux armv8l'), HEADLESS), /X11; Linux x86_64/);
    assert.match(userAgentFor(on('FreeBSD'), HEADLESS), /Windows NT 10\.0/);
    assert.match(userAgentFor(null, HEADLESS), /Windows NT 10\.0/);
  });

  it('only strips Headless when the browser UA has no Chrome version', () => {
    assert.equal(userAgentFor(on('Win32'), 'Something HeadlessChrome'), 'Something Chrome');
    assert.equal(userAgentFor(on('Win32'), undefined), '');
  });
});

describe('metadataFor', () => {
  it("uses the browser's own brand list when it was read", () => {
    const brands = [
      { brand: 'Google Chrome', version: '131' },
      { brand: 'Not_A Brand', version: '8' },
    ];
    const meta = metadataFor(on('Win32'), HEADLESS, brands);
    assert.deepEqual(meta.brands, brands);
    assert.deepEqual(meta.fullVersionList, [
      { brand: 'Google Chrome', version: '131.0.0.0' },
      { brand: 'Not_A Brand', version: '8.0.0.0' },
    ]);
    assert.equal(meta.fullVersion, '131.0.0.0');
  });

  it("falls back to Chrome's own brand list, never an empty one", () => {
    const meta = metadataFor(on('Win32'), HEADLESS, []);
    assert.deepEqual(
      meta.brands.map((b) => b.brand),
      ['Chromium', 'Not?A_Brand', 'Google Chrome'],
    );
  });

  it('assumes Chrome 131 when the UA carries no version', () => {
    assert.equal(metadataFor(on('Win32'), 'curl', null).fullVersion, '131.0.0.0');
  });

  it('keeps a full brand version as it is', () => {
    const meta = metadataFor(on('Win32'), HEADLESS, [{ brand: 'Chromium', version: '131.0.6778.0' }]);
    assert.equal(meta.fullVersionList[0].version, '131.0.6778.0');
  });

  it("reports the platform's client hints and a desktop device", () => {
    const meta = metadataFor(on('Win32'), HEADLESS, null);
    assert.equal(meta.platform, 'Windows');
    assert.equal(meta.architecture, 'x86');
    assert.deepEqual([meta.model, meta.mobile, meta.wow64], ['', false, false]);
  });

  it('reports arm for a Mac with an Apple GPU, and x86 for an Intel Mac', () => {
    assert.equal(
      metadataFor(on('MacIntel', 'ANGLE (Apple, Apple M2, OpenGL 4.1)'), HEADLESS, null).architecture,
      'arm',
    );
    assert.equal(metadataFor(on('MacIntel', 'ANGLE (Intel, Iris)'), HEADLESS, null).architecture, 'x86');
    assert.equal(metadataFor(on('MacIntel'), HEADLESS, null).platform, 'macOS');
  });
});
