/**
 * Unit tests for the routing strategies: how the pool picks one provider
 * among the available ones.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { STRATEGIES, pickBy } from '../../../../src/modules/gateway/strategies.ts';

/** A provider as the pickers see it. */
const p = (name: string, extra: object = {}) => ({
  name,
  priority: 100,
  active: 0,
  maxConcurrent: 10,
  weight: 1,
  latencyMs: null,
  ...extra,
});

describe('pickBy', () => {
  afterEach(() => mock.restoreAll());

  it('offers the five strategies', () => {
    assert.deepEqual(STRATEGIES, ['priority', 'round-robin', 'least-connections', 'latency', 'weighted']);
  });

  it('priority: the lowest number wins, the first on a tie', () => {
    const list = [p('a', { priority: 5 }), p('b', { priority: 1 }), p('c', { priority: 1 })];
    assert.equal(pickBy('priority', list, { rr: 0 }).name, 'b');
  });

  it('round-robin: takes turns and advances the cursor', () => {
    const list = [p('a'), p('b')];
    const cursor = { rr: 0 };
    assert.deepEqual(
      [1, 2, 3].map(() => pickBy('round-robin', list, cursor).name),
      ['a', 'b', 'a'],
    );
    assert.equal(cursor.rr, 3);
  });

  it('least-connections: the lowest share of its own capacity wins', () => {
    const list = [p('a', { active: 2, maxConcurrent: 4 }), p('b', { active: 3, maxConcurrent: 10 })];
    assert.equal(pickBy('least-connections', list, { rr: 0 }).name, 'b');
  });

  it('latency: an unmeasured provider goes first, then the fastest', () => {
    assert.equal(pickBy('latency', [p('a', { latencyMs: 50 }), p('b')], { rr: 0 }).name, 'b');
    assert.equal(pickBy('latency', [p('a', { latencyMs: 50 }), p('b', { latencyMs: 20 })], { rr: 0 }).name, 'b');
  });

  it('weighted: picks in proportion to weight', () => {
    const list = [p('a', { weight: 1 }), p('b', { weight: 3 })];
    mock.method(Math, 'random', () => 0.2);
    assert.equal(pickBy('weighted', list, { rr: 0 }).name, 'a');
    mock.method(Math, 'random', () => 0.5);
    assert.equal(pickBy('weighted', list, { rr: 0 }).name, 'b');
  });

  it('weighted: falls back to the last provider when every weight is zero', () => {
    mock.method(Math, 'random', () => 0.5);
    assert.equal(pickBy('weighted', [p('a', { weight: 0 }), p('b', { weight: 0 })], { rr: 0 }).name, 'a');
  });

  it('falls back to priority for an unknown strategy, even an inherited property name', () => {
    const list = [p('a', { priority: 9 }), p('b', { priority: 2 })];
    assert.equal(pickBy('fastest', list, { rr: 0 }).name, 'b');
    assert.equal(pickBy('toString', list, { rr: 0 }).name, 'b');
  });
});
