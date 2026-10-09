/** Native frame readiness is exact, bounded and disposed without affecting another observer. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { RecordingFrames } from '../../../../src/main/recording/native-frames.ts';
/** A changing native graph drives the same events as the patched engine. */
function fixture() {
  const root = Object.assign(new EventEmitter(), { detached: false, framesInSubtree: [] as any[] });
  root.framesInSubtree = [root];
  const page = Object.assign(new EventEmitter(), {
    mainFrame: root,
    isDestroyed: () => false,
    _supportsOyaFrameLifecycle: () => true,
  });
  const ready: unknown[] = [],
    removed: unknown[] = [],
    failed: unknown[] = [];
  const observer = new RecordingFrames(page as any, {
    ready: (frame) => ready.push(frame),
    removed: (frame) => removed.push(frame),
    failed: (error) => failed.push(error),
  });
  return { root, page, observer, ready, removed, failed };
}
test('subscribes once and reaches an added child before its first DOM ready', () => {
  const f = fixture();
  assert.deepEqual(f.observer.start(), [f.root]);
  f.observer.start();
  assert.equal(f.root.listenerCount('dom-ready'), 1);
  const child = Object.assign(new EventEmitter(), { detached: false });
  f.root.framesInSubtree.push(child);
  f.page.emit('frame-created', {}, { frame: child });
  child.emit('dom-ready');
  assert.deepEqual(f.ready, [child]);
  f.root.framesInSubtree = [f.root];
  f.page.emit('oya-frame-tree-changed');
  child.emit('dom-ready');
  assert.deepEqual(f.removed, [child]);
  assert.deepEqual(f.ready, [child]);
  f.observer.stop();
  f.root.emit('dom-ready');
  assert.equal(f.page.listenerCount('frame-created'), 0);
  assert.throws(() => f.observer.start(), /stopped/);
});
test('foreign listeners survive native recording disposal', () => {
  const f = fixture();
  const listener = () => {};
  f.root.on('dom-ready', listener);
  f.observer.start();
  f.page.emit('destroyed');
  assert.deepEqual(f.root.listeners('dom-ready'), [listener]);
  assert.equal(f.page.listenerCount('oya-frame-tree-changed'), 0);
});
