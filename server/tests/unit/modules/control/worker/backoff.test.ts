/**
 * Unit tests for the retry backoff shared by provider cleanup and webhook delivery.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { backoff } from '../../../../../src/modules/control/worker/backoff.ts';

describe('backoff', () => {
  it('starts at a second and doubles per attempt', () => {
    assert.deepEqual(
      [0, 1, 2, 3].map((n) => backoff(n, 10, Infinity)),
      [1000, 2000, 4000, 8000],
    );
  });

  it('stops doubling after the maximum exponent', () => {
    assert.equal(backoff(20, 3, Infinity), 8000);
  });

  it('never waits longer than the cap', () => {
    assert.equal(backoff(10, 10, 5000), 5000);
  });
});
