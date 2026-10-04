/**
 * Unit tests for envelope encryption: a sealed value opens only under the
 * scope it was sealed with, tampering is detected, and a missing
 * OYA_PROFILE_SECRET is generated into the data directory.
 */
import { describe, it, before, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import {
  derivedKey,
  haveSecret,
  isSealed,
  open,
  openBytes,
  openText,
  rewrap,
  rewrapText,
  seal,
  sealBytes,
  sealText,
} from '../../../src/platform/secrets.ts';
import { KEY_ID_BYTES } from '../../../src/platform/constants.ts';
import { dataPath } from '../../../src/platform/paths.ts';

// Most rules here are about the keyed format; the default (unkeyed, for rollouts) has its own test.
process.env.OYA_SEAL_KEY_IDS = 'true';

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

  it('starts every sealed buffer with the keyed format version when key ids are on', () => {
    assert.equal(seal('s', 'x')[0], 2);
  });

  it('writes the unkeyed format by default, so a release before key ids can still open it', () => {
    delete process.env.OYA_SEAL_KEY_IDS;
    try {
      const sealed = seal('s', 'x');
      assert.equal(sealed[0], 1);
      assert.equal(open('s', sealed), 'x');
    } finally {
      process.env.OYA_SEAL_KEY_IDS = 'true';
    }
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

  it('round-trips raw bytes without a JSON detour', () => {
    const jpeg = Buffer.from([0xff, 0xd8, 0xff, 0x00, 0x01]);
    assert.deepEqual(openBytes('rec:1', sealBytes('rec:1', jpeg)), jpeg);
  });

  it('tells a sealed buffer from plaintext written before sealing', () => {
    assert.equal(isSealed(seal('s', 'x')), true);
    assert.equal(isSealed(Buffer.from('{"a":1}')), false);
    assert.equal(isSealed(Buffer.from([0xff, 0xd8])), false);
  });

  it('opens a record in the unkeyed first format', () => {
    const keyed = seal('s', { a: 1 });
    const unkeyed = Buffer.concat([Buffer.from([1]), keyed.subarray(1 + KEY_ID_BYTES)]);
    assert.deepEqual(open('s', unkeyed), { a: 1 });
  });
});

describe('secrets: rotation', () => {
  const saved = { ...process.env };
  afterEach(() => {
    for (const name of ['OYA_PROFILE_SECRET', 'OYA_PROFILE_SECRET_PREVIOUS', 'OYA_PROFILE_SALT_PREVIOUS']) {
      if (saved[name] === undefined) delete process.env[name];
      else process.env[name] = saved[name];
    }
  });

  /** Makes `secret` current and `previousSecret` (if any) the outgoing key. */
  const keys = (secret: string, previousSecret?: string) => {
    process.env.OYA_PROFILE_SECRET = secret;
    if (previousSecret) process.env.OYA_PROFILE_SECRET_PREVIOUS = previousSecret;
    else delete process.env.OYA_PROFILE_SECRET_PREVIOUS;
  };

  it('opens a record sealed under the previous key after a rotation', () => {
    keys('old-secret');
    const sealed = seal('s', 'v');
    keys('new-secret', 'old-secret');
    assert.equal(open('s', sealed), 'v');
  });

  it('opens an unkeyed record sealed under the previous key', () => {
    keys('old-secret');
    const keyed = seal('s', 'v');
    const unkeyed = Buffer.concat([Buffer.from([1]), keyed.subarray(1 + KEY_ID_BYTES)]);
    keys('new-secret', 'old-secret');
    assert.equal(open('s', unkeyed), 'v');
  });

  it('refuses a record sealed under a key the server no longer holds', () => {
    keys('old-secret');
    const sealed = seal('s', 'v');
    keys('new-secret');
    assert.throws(() => open('s', sealed), /does not hold/);
  });

  it('rewraps a record onto the current key, so it opens once the previous key is dropped', () => {
    keys('old-secret');
    const sealed = sealText('s', 'v');
    keys('new-secret', 'old-secret');
    const moved = rewrapText('s', sealed);
    keys('new-secret');
    assert.equal(openText('s', moved), 'v');
  });

  it('leaves a record already under the current key unchanged', () => {
    keys('new-secret');
    const sealed = seal('s', 'v');
    assert.equal(rewrap('s', sealed), sealed);
  });

  it('refuses to rewrap under the wrong scope', () => {
    keys('old-secret');
    const sealed = seal('a', 'v');
    keys('new-secret', 'old-secret');
    assert.throws(() => rewrap('b', sealed));
  });

  it('keeps a derived key unchanged when the secret moves to previous', () => {
    keys('old-secret');
    const before = derivedKey('audit');
    keys('new-secret', 'old-secret');
    assert.deepEqual(derivedKey('audit'), before);
    assert.notDeepEqual(derivedKey('other'), before);
  });
});
