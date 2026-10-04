/**
 * Unit tests for roving focus's index arithmetic.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { rovingIndex, isRovingKey } from '../../../../src/renderer/hooks/use-roving-focus.ts';

describe('rovingIndex', () => {
  it('steps with the arrows, wrapping, and jumps with Home and End', () => {
    assert.equal(rovingIndex('ArrowRight', 2, 3), 0);
    assert.equal(rovingIndex('ArrowLeft', 0, 3), 2);
    assert.equal(rovingIndex('Home', 2, 3), 0);
    assert.equal(rovingIndex('End', 0, 3), 2);
  });

  it('knows only the roving keys', () => {
    assert.equal(isRovingKey('ArrowLeft'), true);
    assert.equal(isRovingKey('Enter'), false);
  });
});
