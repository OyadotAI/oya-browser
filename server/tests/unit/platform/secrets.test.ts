/**
 * Unit tests for envelope encryption: a sealed value opens only under the
 * scope it was sealed with, tampering is detected, and a missing
 * OYA_PROFILE_SECRET is generated into the data directory.
 */
import { describe, it, before, mock } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { haveSecret, open, openText, seal, sealText } from '../../../src/platform/secrets.ts';
import { dataPath } from '../../../src/platform/paths.ts';

describe('secrets', () => {
  before(() => {
    mock.method(console, 'warn', () => {});
  });

  it(
    'generates a secret in the data directory when none is configured',
    { skip: !!process.env.OYA_PROFILE_SECRET },
    () => {
      assert.equal(haveSecret(), true);
      assert.equal(existsSync(dataPath('.secret')), true);
    },
  );

  it('opens what it sealed under the same scope', () => {
    const value = { user: 'ada', password: 'hunter2', n: 3 };
    assert.deepEqual(open('proxy:p-1', seal('proxy:p-1', value)), value);
  });

  it('starts every sealed buffer with the format version', () => {
    assert.equal(seal('s', 'x')[0], 1);
  });

  it('seals the same value differently each time', () => {
    assert.notDeepEqual(seal('s', 'same'), seal('s', 'same'));
  });

  it('refuses to open a ciphertext copied into another scope', () => {
    const sealed = seal('tenant-a:mfa', 'seed');
    assert.throws(() => open('tenant-b:mfa', sealed));
  });

  it('refuses a ciphertext that was altered', () => {
    const sealed = seal('s', 'secret');
    sealed[sealed.length - 1] ^= 1;
    assert.throws(() => open('s', sealed));
  });

  it('refuses an unknown format version', () => {
    const sealed = seal('s', 'secret');
    sealed[0] = 9;
    assert.throws(() => open('s', sealed), /Unsupported sealed format/);
  });

  it('round-trips through base64 text for JSON records', () => {
    const text = sealText('s', ['a', 1]);
    assert.equal(typeof text, 'string');
    assert.deepEqual(openText('s', text), ['a', 1]);
  });
});
