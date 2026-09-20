/**
 * Unit tests for Provider: its defaults, capacity, failure backoff and the
 * latency moving average.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { Provider } from '../../../../src/modules/gateway/provider.ts';
import {
  COOLDOWN_BASE_MS,
  COOLDOWN_MAX_MS,
  DEFAULT_MAX_CONCURRENT,
  DEFAULT_PRIORITY,
} from '../../../../src/modules/gateway/constants.ts';

describe('Provider', () => {
  afterEach(() => mock.timers.reset());

  it('defaults to a shared, enabled cdp provider with the default limits', () => {
    const p = new Provider({ name: 'chrome' });
    assert.deepEqual(
      [p.owner, p.type, p.wsUrl, p.enabled, p.maxConcurrent, p.priority, p.weight],
      [null, 'cdp', null, true, DEFAULT_MAX_CONCURRENT, DEFAULT_PRIORITY, 1],
    );
  });

  it('keeps a deliberate zero rather than the default', () => {
    const p = new Provider({ name: 'x', priority: 0, maxConcurrent: 0 });
    assert.deepEqual([p.priority, p.maxConcurrent], [0, 0]);
  });

  it('is available while enabled, out of cooldown and below capacity', () => {
    const p = new Provider({ name: 'x', maxConcurrent: 1 });
    assert.equal(p.available, true);
    p.active = 1;
    assert.deepEqual([p.hasCapacity, p.available], [false, false]);
    assert.equal(new Provider({ name: 'y', enabled: false }).healthy, false);
  });

  it('cools down for 5s after a failure, doubling up to 5 minutes', () => {
    mock.timers.enable({ apis: ['Date'], now: 0 });
    const p = new Provider({ name: 'x' });
    p.fail();
    assert.equal(p.cooldownUntil, COOLDOWN_BASE_MS);
    p.fail();
    assert.equal(p.cooldownUntil, COOLDOWN_BASE_MS * 2);
    for (let i = 0; i < 20; i++) p.fail();
    assert.equal(p.cooldownUntil, COOLDOWN_MAX_MS);
    assert.equal(p.totalFailures, 22);
  });

  it('is healthy again once the cooldown passes', () => {
    mock.timers.enable({ apis: ['Date'], now: 0 });
    const p = new Provider({ name: 'x' });
    p.fail();
    assert.equal(p.healthy, false);
    mock.timers.tick(COOLDOWN_BASE_MS);
    assert.equal(p.healthy, true);
  });

  it('clears the backoff on success and averages latency', () => {
    const p = new Provider({ name: 'x' });
    p.fail();
    p.succeed(100);
    assert.deepEqual([p.failures, p.cooldownUntil, p.latencyMs], [0, 0, 100]);
    p.succeed(200);
    assert.equal(p.latencyMs, 130);
    p.succeed(NaN);
    assert.equal(p.latencyMs, 130);
  });

  it('reports itself with derived health and rounded latency', () => {
    mock.timers.enable({ apis: ['Date'], now: 0 });
    const p = new Provider({ name: 'x', owner: 'o', wsUrl: 'ws://h' });
    p.succeed(12.6);
    p.fail();
    assert.deepEqual(p.toJSON(), {
      name: 'x',
      type: 'cdp',
      enabled: true,
      owner: 'o',
      shared: false,
      active: 0,
      maxConcurrent: DEFAULT_MAX_CONCURRENT,
      weight: 1,
      priority: DEFAULT_PRIORITY,
      healthy: false,
      available: false,
      latencyMs: 13,
      cooldownMsRemaining: COOLDOWN_BASE_MS,
      totalSessions: 0,
      totalFailures: 1,
    });
  });
});
