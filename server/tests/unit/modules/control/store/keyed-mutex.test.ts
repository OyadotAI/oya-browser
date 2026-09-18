/**
 * Unit tests for KeyedMutex: per-key FIFO turns for in-process transactions.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { KeyedMutex } from '../../../../../src/modules/control/store/keyed-mutex.ts';

/** Lets pending promise callbacks run. */
const flush = () => new Promise((resolve) => setImmediate(resolve));

describe('KeyedMutex', () => {
  it('grants a free key at once, saying nobody was ahead', async () => {
    const { release, waited } = await new KeyedMutex().acquire('a');
    assert.equal(waited, false);
    release();
  });

  it('makes a second holder of the same key wait until the first releases', async () => {
    const mutex = new KeyedMutex();
    const first = await mutex.acquire('a');
    let second = null;
    void mutex.acquire('a').then((grant) => (second = grant));
    await flush();
    assert.equal(second, null, 'still waiting');
    first.release();
    await flush();
    assert.equal(second.waited, true);
    second.release();
  });

  it('serves waiters in the order they arrived', async () => {
    const mutex = new KeyedMutex(),
      order = [];
    const first = await mutex.acquire('a');
    const b = mutex.acquire('a').then((g) => (order.push('b'), g));
    const c = mutex.acquire('a').then((g) => (order.push('c'), g));
    first.release();
    (await b).release();
    (await c).release();
    assert.deepEqual(order, ['b', 'c']);
  });

  it('does not make different keys wait on each other', async () => {
    const mutex = new KeyedMutex();
    const a = await mutex.acquire('a');
    const b = await mutex.acquire('b');
    assert.equal(b.waited, false);
    a.release();
    b.release();
  });

  it('forgets a key once it is released with nobody behind', async () => {
    const mutex = new KeyedMutex();
    (await mutex.acquire('a')).release();
    await flush();
    assert.equal(mutex.tails.size, 0);
  });
});
