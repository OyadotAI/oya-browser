/**
 * Unit tests for a past time in words.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ago } from '../../../../src/renderer/ui/ago.ts';

const MINUTE = 60_000;

describe('ago', () => {
  it('tells a past time in words', () => {
    const at = (ms: number) => ago(1e12 - ms, 1e12);
    assert.deepEqual(
      [at(10_000), at(5 * MINUTE), at(26 * 60 * MINUTE), at(3 * 24 * 60 * MINUTE)],
      ['just now', '5 minutes ago', 'yesterday', '3 days ago'],
    );
  });
});
