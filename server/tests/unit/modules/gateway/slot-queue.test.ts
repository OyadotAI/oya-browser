/**
 * Unit tests for SlotQueue: callers wait oldest first for a provider slot,
 * give up at their timeout, and are only handed providers they may use.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { SlotQueue } from '../../../../src/modules/gateway/slot-queue.ts';

describe('SlotQueue', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
  afterEach(() => mock.timers.reset());

  it('hands a freed provider to the oldest waiter that may use it', async () => {
    const q = new SlotQueue();
    const first = q.wait(1000, 'a', undefined, new Set());
    q.wait(1000, 'a', undefined, new Set());
    assert.equal(q.depth, 2);
    const w = q.take({ owner: 'a' });
    w.resolve('provider');
    assert.equal(await first, 'provider');
    assert.equal(q.depth, 1);
  });

  it("skips waiters whose owner cannot use another owner's provider", () => {
    const q = new SlotQueue();
    q.wait(1000, 'b', undefined, new Set());
    q.wait(1000, 'a', undefined, new Set());
    assert.equal(q.take({ owner: 'a' }).owner, 'a');
    assert.equal(q.take({ owner: 'a' }), null);
  });

  it('lets any waiter use a shared provider', () => {
    const q = new SlotQueue();
    q.wait(1000, 'b', undefined, new Set());
    assert.equal(q.take({ owner: null }).owner, 'b');
  });

  it('resolves null and leaves the line when the wait times out', async () => {
    const q = new SlotQueue();
    const waiting = q.wait(1000, 'a', undefined, new Set());
    mock.timers.tick(1000);
    assert.equal(await waiting, null);
    assert.equal(q.depth, 0);
  });

  it('stops the timer once handed a provider', async () => {
    const q = new SlotQueue();
    const waiting = q.wait(1000, 'a', undefined, new Set());
    q.take({ owner: 'a' }).resolve('p');
    mock.timers.tick(1000);
    assert.equal(await waiting, 'p');
  });
});
