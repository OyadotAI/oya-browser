/** Bounded consent serialization keeps independent decisions and releases every waiting callback. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { MediaApprovalQueue } from '../../../../src/main/app/media-queue.ts';
import { MEDIA_PERMISSION_WAITERS, MEDIA_PERMISSION_WAIT_MS } from '../../../../src/main/app/constants.ts';

test('denying a request refuses queued duplicates without repeating consent or denying other devices', async () => {
  const queue = new MediaApprovalQueue();
  const pending = Promise.withResolvers<boolean>();
  const first = queue.run(
    () => true,
    () => pending.promise,
    'document:audio',
  );
  const duplicate = queue.run(
    () => true,
    async () => assert.fail('duplicate prompt'),
    'document:audio',
  );
  const independent = queue.run(
    () => true,
    async () => true,
    'document:screen',
  );
  pending.resolve(false);
  assert.deepEqual(await Promise.all([first, duplicate, independent]), [false, false, true]);
  assert.equal(
    await queue.run(
      () => true,
      async () => true,
      'document:audio',
    ),
    true,
    'later retry is independent',
  );
});

test('a grant still runs each duplicate request through its own authorization', async () => {
  const queue = new MediaApprovalQueue();
  const pending = Promise.withResolvers<boolean>();
  const first = queue.run(
    () => true,
    () => pending.promise,
    'same',
  );
  let prompts = 0;
  const second = queue.run(
    () => true,
    async () => (prompts++, true),
    'same',
  );
  pending.resolve(true);
  assert.deepEqual(await Promise.all([first, second]), [true, true]);
  assert.equal(prompts, 1);
});

test('concurrent media requests wait in order and receive their own decision', async () => {
  const queue = new MediaApprovalQueue();
  const pending = Promise.withResolvers<boolean>();
  const order: string[] = [];
  const first = queue.run(
    () => true,
    () => pending.promise,
  );
  const second = queue.run(
    () => true,
    async () => (order.push('second'), false),
  );
  const third = queue.run(
    () => true,
    async () => (order.push('third'), true),
  );
  assert.deepEqual(order, []);
  pending.resolve(true);
  assert.deepEqual(await Promise.all([first, second, third]), [true, false, true]);
  assert.deepEqual(order, ['second', 'third']);
});

test('a burst beyond the bounded queue is refused without running another prompt', async () => {
  const queue = new MediaApprovalQueue();
  const pending = Promise.withResolvers<boolean>();
  const active = queue.run(
    () => true,
    () => pending.promise,
  );
  const waiting = Array.from({ length: MEDIA_PERMISSION_WAITERS }, () =>
    queue.run(
      () => true,
      async () => true,
    ),
  );
  assert.equal(
    await queue.run(
      () => true,
      async () => assert.fail('overflow prompt'),
    ),
    false,
  );
  pending.resolve(true);
  assert.deepEqual(await Promise.all([active, ...waiting]), Array(MEDIA_PERMISSION_WAITERS + 1).fill(true));
});

test('waiting requests expire without releasing or duplicating the active native dialog', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const queue = new MediaApprovalQueue();
  const pending = Promise.withResolvers<boolean>();
  const active = queue.run(
    () => true,
    () => pending.promise,
  );
  const waiting = queue.run(
    () => true,
    async () => assert.fail('expired prompt'),
  );
  t.mock.timers.tick(MEDIA_PERMISSION_WAIT_MS);
  assert.equal(await waiting, false);
  let started = false;
  const retry = queue.run(
    () => true,
    async () => (started = true),
  );
  assert.equal(started, false);
  pending.resolve(true);
  assert.deepEqual(await Promise.all([active, retry]), [true, true]);
});

test('a queued request that loses its original document cannot start a prompt', async () => {
  const queue = new MediaApprovalQueue();
  const pending = Promise.withResolvers<boolean>();
  const active = queue.run(
    () => true,
    () => pending.promise,
  );
  let current = true;
  const waiting = queue.run(
    () => current,
    async () => assert.fail('stale prompt'),
  );
  current = false;
  pending.resolve(true);
  assert.deepEqual(await Promise.all([active, waiting]), [true, false]);
});

test('throws and rejections release the slot while stale results remain refused', async () => {
  const queue = new MediaApprovalQueue();
  assert.equal(
    await queue.run(
      () => true,
      () => {
        throw Error('native failure');
      },
    ),
    false,
  );
  assert.equal(
    await queue.run(
      () => true,
      async () => {
        throw Error('OS failure');
      },
    ),
    false,
  );
  let current = true;
  assert.equal(
    await queue.run(
      () => current,
      async () => {
        current = false;
        return true;
      },
    ),
    false,
  );
  assert.equal(
    await queue.run(
      () => true,
      async () => true,
    ),
    true,
  );
});
