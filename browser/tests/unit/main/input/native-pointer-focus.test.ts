/** Native pointer focus provenance is exact, synchronous and revocable on every exit. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import {
  withNativePointerFocus,
  isNativePointerFocus,
  installPointerFocusFence,
} from '../../../../src/main/input/native-pointer-focus.ts';

it('exempts only the marked contents while preserving normal human-control policy', () => {
  const target = new EventEmitter(),
    other = new EventEmitter();
  let reclaimed = 0,
    human = false;
  for (const contents of [target, other])
    installPointerFocusFence(
      contents,
      () => human,
      () => reclaimed++,
    );
  target.emit('focus');
  assert.equal(reclaimed, 1);
  withNativePointerFocus(target, () => {
    target.emit('focus');
    other.emit('focus');
    assert.equal(reclaimed, 2);
  });
  target.emit('focus');
  assert.equal(reclaimed, 3);
  human = true;
  target.emit('focus');
  assert.equal(reclaimed, 3);
});
it('nested focus retains the caller marker and exceptions always revoke new provenance', () => {
  const target = {};
  assert.throws(
    () =>
      withNativePointerFocus(target, () => {
        assert.throws(
          () =>
            withNativePointerFocus(target, () => {
              throw Error('nested');
            }),
          /nested/,
        );
        assert.equal(isNativePointerFocus(target), true);
        throw Error('outer');
      }),
    /outer/,
  );
  assert.equal(isNativePointerFocus(target), false);
});
it('deferred focus reclamation and promise continuations cannot inherit provenance', async () => {
  const target = new EventEmitter();
  let reclaimed = 0;
  installPointerFocusFence(
    target,
    () => false,
    () => reclaimed++,
  );
  const pending = withNativePointerFocus(target, () =>
    Promise.resolve().then(() => {
      assert.equal(isNativePointerFocus(target), false);
      target.emit('focus');
    }),
  );
  assert.equal(isNativePointerFocus(target), false);
  await pending;
  assert.equal(reclaimed, 1);
});
