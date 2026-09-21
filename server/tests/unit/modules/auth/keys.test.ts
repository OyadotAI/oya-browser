/**
 * Unit tests for which API keys exist: env admin keys, the fleet token and the
 * digests of stored keys, and how keys are minted, hashed and shown.
 */
import { describe, it, after } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { restoreEnv } from '../../support/data-dir.ts';

// Both are read once, when the module loads.
const saved = { keys: process.env.API_KEYS, fleet: process.env.FLEET_TOKEN };
process.env.API_KEYS = ' env-key-1 , env-key-2,, ';
process.env.FLEET_TOKEN = ' fleet-token ';
const keys = await import('../../../../src/modules/auth/keys.ts');
after(() => {
  restoreEnv('API_KEYS', saved.keys);
  restoreEnv('FLEET_TOKEN', saved.fleet);
});

describe('key digests and display', () => {
  it('stores a key as its sha256 hex digest', () => {
    assert.equal(keys.keyDigest('abc'), createHash('sha256').update('abc').digest('hex'));
    assert.equal(keys.keyDigest(123), keys.keyDigest('123'));
  });

  it('shows only the first eight characters of a key', () => {
    assert.equal(keys.keyPrefix('abcdefghijkl'), 'abcdefgh');
  });

  it('mints distinct 32-character URL-safe keys', () => {
    const a = keys.generateKey();
    assert.match(a, /^[A-Za-z0-9_-]{32}$/);
    assert.notEqual(a, keys.generateKey());
  });
});

describe('known keys', () => {
  it('reads API_KEYS as a trimmed comma list, skipping blanks', () => {
    assert.equal(keys.isEnvKey('env-key-1'), true);
    assert.equal(keys.isEnvKey('env-key-2'), true);
    assert.equal(keys.isEnvKey(''), false);
  });

  it('knows the trimmed fleet token and nothing else as one', () => {
    assert.equal(keys.isFleetToken('fleet-token'), true);
    assert.equal(keys.isFleetToken(' fleet-token '), false);
    assert.equal(keys.isFleetToken('env-key-1'), false);
  });

  it('lists the keys held in the clear: env keys and the fleet token', () => {
    assert.deepEqual(keys.knownKeys().sort(), ['env-key-1', 'env-key-2', 'fleet-token']);
  });
});

describe('validateApiKey', () => {
  it('accepts env keys and the fleet token', () => {
    assert.equal(keys.validateApiKey('env-key-2'), true);
    assert.equal(keys.validateApiKey('fleet-token'), true);
  });

  it('accepts a stored key by its digest, and only while the digest is cached', () => {
    const key = keys.generateKey();
    assert.equal(keys.validateApiKey(key), false);
    keys.keyCache.add(keys.keyDigest(key));
    assert.equal(keys.validateApiKey(key), true);
    keys.keyCache.delete(keys.keyDigest(key));
  });

  it('refuses an empty key', () => {
    assert.equal(keys.validateApiKey(''), false);
    assert.equal(keys.validateApiKey(undefined), false);
  });

  it('is ready at once without a database', async () => {
    assert.equal(await keys.authReady, true);
  });
});
