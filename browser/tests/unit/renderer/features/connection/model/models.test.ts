/**
 * Unit tests for the connection feature's shared helpers: reading payloads,
 * and an error's words.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { messageOf, shape, shapeOrNull } from '../../../../../../src/renderer/features/connection/model/models.ts';

describe('payloads', () => {
  it('reads nothing as an empty shape, or null when asked', () => {
    assert.deepEqual(shape(null), {});
    assert.equal(shapeOrNull(undefined), null);
    assert.deepEqual(shapeOrNull({ a: 1 }), { a: 1 });
  });

  it("uses an error's message, or the fallback", () => {
    assert.equal(messageOf(new Error('boom'), 'x'), 'boom');
    assert.equal(messageOf('boom', 'x'), 'x');
  });
});
