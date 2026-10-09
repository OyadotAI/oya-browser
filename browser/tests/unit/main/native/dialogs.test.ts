/** Native dialog subscriptions retain ownership until safe disposal and never attach a debugger. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { watchNativeDialogs } from '../../../../src/main/native/index.ts';
/** Minimal engine seam with an explicit setter and lifecycle. */
function fixture() {
  const handlers: unknown[] = [];
  const page = Object.assign(new EventEmitter(), {
    isDestroyed: () => false,
    _oyaBeforeUnloadDialogs: true,
    _setOyaDialogHandler: (handler: unknown) => {
      handlers.push(handler);
    },
  });
  Object.defineProperty(page, 'debugger', {
    get() {
      assert.fail('No internal CDP');
    },
  });
  return { page, handlers };
}
test('unsupported engine fails closed', () => {
  const page = Object.assign(new EventEmitter(), { isDestroyed: () => false });
  assert.throws(
    () =>
      watchNativeDialogs(
        page,
        () => {},
        () => {},
      ),
    /does not support/,
  );
});
test('destroyed surface refuses registration', () => {
  const f = fixture();
  f.page.isDestroyed = () => true;
  assert.throws(
    () =>
      watchNativeDialogs(
        f.page,
        () => {},
        () => {},
      ),
    /destroyed/,
  );
  assert.equal(f.handlers.length, 0);
});
test('cancellation reports the exact reply and disposal is idempotent', () => {
  const f = fixture();
  const cancelled: unknown[] = [];
  const handler = () => {};
  const stop = watchNativeDialogs(f.page, handler, (reply) => cancelled.push(reply));
  const reply = () => {};
  f.page.emit('-oya-dialog-cancelled', reply);
  assert.deepEqual(cancelled, [reply]);
  stop();
  stop();
  assert.deepEqual(f.handlers, [handler, null]);
  assert.equal(f.page.listenerCount('-oya-dialog-cancelled'), 0);
});
test('pending refusal retains cancellation until disposal can succeed', () => {
  const f = fixture();
  const cancelled: unknown[] = [];
  const stop = watchNativeDialogs(
    f.page,
    () => {},
    (reply) => cancelled.push(reply),
  );
  f.page._setOyaDialogHandler = () => {
    throw new Error('pending');
  };
  assert.throws(stop, /pending/);
  assert.equal(f.page.listenerCount('-oya-dialog-cancelled'), 1);
  f.page.isDestroyed = () => true;
  stop();
  assert.equal(f.page.listenerCount('-oya-dialog-cancelled'), 0);
});

test('engines missing unload decisions fail before installing a handler', () => {
  const f = fixture();
  f.page._oyaBeforeUnloadDialogs = false;
  assert.throws(
    () =>
      watchNativeDialogs(
        f.page,
        () => {},
        () => {},
      ),
    /before-unload/,
  );
  assert.deepEqual(f.handlers, []);
  assert.equal(f.page.listenerCount('-oya-dialog-cancelled'), 0);
});
