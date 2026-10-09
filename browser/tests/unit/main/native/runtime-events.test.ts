/** Native context notifications cannot escape document ordering, human ownership or disposal. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { RuntimeEvents } from '../../../../src/main/native/runtime-events.ts';
/** Deferred native replies let tests control navigation races without wall-clock timers. */
function fixture() {
  const wc = new EventEmitter(),
    events: any[] = [],
    pending: Array<(context: any) => void> = [];
  let allowed = true;
  const stop = new RuntimeEvents({
    page: {
      webContents: Object.assign(wc, {
        _supportsOyaFrameLifecycle: () => true,
        isDestroyed: () => false,
        mainFrame: { framesInSubtree: [] },
      }),
    } as any,
    allowed: () => allowed,
    emit: (...args) => events.push(args),
    current: () =>
      new Promise((resolve) => pending.push((context) => resolve(Array.isArray(context) ? context : [context]))),
  }).start();
  const context = (id: number) => ({
    id,
    uniqueId: `native-${id}`,
    target: 'page',
    frameId: 'page',
    frame: { origin: 'https://example.test' },
  });
  return {
    wc,
    events,
    pending,
    stop,
    context,
    human: () => {
      allowed = false;
    },
  };
}
test('out-of-order native context replies cannot announce a replaced document', async () => {
  const f = fixture();
  f.wc.emit('did-navigate');
  f.wc.emit('dom-ready');
  f.pending[1](f.context(2));
  f.pending[0](f.context(1));
  await Promise.resolve();
  assert.equal(f.events.length, 1);
  assert.equal(f.events[0][1].context.id, 2);
  f.stop();
});
test('human ownership suppresses context replies already in flight', async () => {
  const f = fixture();
  f.human();
  f.pending[0](f.context(1));
  await Promise.resolve();
  assert.equal(f.events.length, 0);
  f.wc.emit('dom-ready');
  assert.equal(f.pending.length, 1);
  f.stop();
});
test('disposal revokes pending context notifications and removes only owned listeners', async () => {
  const f = fixture();
  const other = () => {};
  f.wc.on('dom-ready', other);
  f.stop();
  f.pending[0](f.context(1));
  await Promise.resolve();
  assert.equal(f.events.length, 0);
  assert.deepEqual(f.wc.listeners('dom-ready'), [other]);
  assert.equal(f.wc.listenerCount('did-navigate'), 0);
});

test('child removal destroys only that context and keeps a surviving sibling announced', async () => {
  const f = fixture();
  const first = f.context(1),
    sibling = f.context(2);
  f.pending[0]([first, sibling]);
  await Promise.resolve();
  f.wc.emit('oya-frame-tree-changed');
  f.pending[1]([sibling]);
  await Promise.resolve();
  assert.equal(f.events.length, 3);
  assert.deepEqual(f.events[2], [
    'Runtime.executionContextDestroyed',
    { executionContextId: 1, executionContextUniqueId: 'native-1' },
  ]);
  f.stop();
  assert.equal(f.wc.listenerCount('oya-frame-tree-changed'), 0);
  assert.equal(f.wc.listenerCount('frame-created'), 0);
});
