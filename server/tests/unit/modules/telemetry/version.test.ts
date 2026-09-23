/**
 * Unit tests for reading the desktop's reported version: a release number is
 * kept, anything a client made up reads as unknown.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { versionOf } from '../../../../src/modules/telemetry/version.ts';

describe('versionOf', () => {
  it('keeps a release number, with or without a short pre-release tag', () => {
    assert.equal(versionOf('1.0.115'), '1.0.115');
    assert.equal(versionOf(' 2.3.4-beta.1 '), '2.3.4-beta.1');
  });

  it('reads anything else as unknown', () => {
    for (const reported of [
      undefined,
      null,
      1.0,
      '',
      'latest',
      '1.0',
      '<!channel>',
      '1.0.115; rm -rf',
      'x'.repeat(200),
    ]) {
      assert.equal(versionOf(reported), 'unknown', String(reported));
    }
  });
});
