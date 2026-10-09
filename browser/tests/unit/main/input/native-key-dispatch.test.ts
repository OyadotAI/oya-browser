/** Browser-key provenance must never turn into a global or long-lived input bypass. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { isNativeKeyDispatch, sendNativeKey } from '../../../../src/main/input/native-key-dispatch.ts';

const event = { type: 'keyDown' as const, keyCode: 'x' };

it('grants only the target contents during synchronous native dispatch', () => {
  const other = {};
  const contents = {
    sendInputEvent() {
      assert.equal(isNativeKeyDispatch(contents), true);
      assert.equal(isNativeKeyDispatch(other), false);
    },
  };
  assert.equal(isNativeKeyDispatch(contents), false);
  sendNativeKey(contents, event);
  assert.equal(isNativeKeyDispatch(contents), false);
});

it('revokes the grant after native dispatch throws', () => {
  const contents = {
    sendInputEvent() {
      throw Error('dispatch failed');
    },
  };
  assert.throws(() => sendNativeKey(contents, event), /dispatch failed/);
  assert.equal(isNativeKeyDispatch(contents), false);
});

it('nested dispatch restores the outer scope and leaves no grant afterwards', () => {
  let nesting = false;
  const contents = {
    sendInputEvent() {
      if (nesting) return;
      nesting = true;
      sendNativeKey(contents, event);
      assert.equal(isNativeKeyDispatch(contents), true);
    },
  };
  sendNativeKey(contents, event);
  assert.equal(isNativeKeyDispatch(contents), false);
});

it('no grant survives into asynchronous callbacks', async () => {
  let pending: Promise<void> | undefined;
  const contents = {
    sendInputEvent() {
      pending = Promise.resolve().then(() => assert.equal(isNativeKeyDispatch(contents), false));
    },
  };
  sendNativeKey(contents, event);
  await pending;
});
