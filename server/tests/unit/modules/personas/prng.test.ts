/**
 * Unit tests for the seeded randomness fingerprints are drawn from: the LCG
 * and string hash must match the browser side value for value, so these pin
 * their output.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { createPRNG, seedFromString, pick } from '../../../../src/modules/personas/prng.ts';

describe('createPRNG', () => {
  it('gives the same stream for the same seed', () => {
    const a = createPRNG(7);
    const b = createPRNG(7);
    assert.deepEqual([a(), a(), a()], [b(), b(), b()]);
  });

  it('keeps producing the values the browser side derives from a seed', () => {
    assert.equal(createPRNG(1)(), 0.23645552532664862);
  });

  it('stays within [0, 1]', () => {
    const rng = createPRNG(123456789);
    for (let i = 0; i < 1000; i++) {
      const v = rng();
      assert.ok(v >= 0 && v <= 1, `${v} out of range`);
    }
  });
});

describe('seedFromString', () => {
  it('hashes a string to the same unsigned 32-bit seed every time', () => {
    assert.equal(seedFromString('abc'), 96354);
    assert.equal(seedFromString('abc'), seedFromString('abc'));
  });

  it('hashes the empty string to zero', () => {
    assert.equal(seedFromString(''), 0);
  });

  it('never returns a negative seed, even when the hash overflows', () => {
    const seed = seedFromString('a long string that overflows a signed 32-bit hash many times over');
    assert.ok(seed >= 0 && seed <= 0xffffffff);
  });
});

describe('pick', () => {
  it('picks by where the random value falls in the list', () => {
    assert.equal(
      pick(['a', 'b', 'c'], () => 0),
      'a',
    );
    assert.equal(
      pick(['a', 'b', 'c'], () => 0.5),
      'b',
    );
    assert.equal(
      pick(['a', 'b', 'c'], () => 0.99),
      'c',
    );
  });
});
