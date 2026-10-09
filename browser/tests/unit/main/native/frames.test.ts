/** Native frame capability errors must never trigger debugging or main-world fallbacks. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { evaluateFrame } from '../../../../src/main/native/index.ts';
test('executes on the exact frame without granting a user gesture', async () => {
  const frame = {
    detached: false,
    async _executeJavaScriptInOyaWorld(code: string, gesture?: boolean) {
      assert.equal(this, frame);
      assert.equal(code, 'Promise.resolve(42)');
      assert.equal(gesture, false);
      return 42;
    },
  };
  assert.equal(await evaluateFrame(frame, 'Promise.resolve(42)'), 42);
});
test('unsupported engines fail closed', async () => {
  await assert.rejects(evaluateFrame({ detached: false }, '42'), /does not support/);
});
test('detached frames never execute', async () => {
  await assert.rejects(
    evaluateFrame(
      {
        detached: true,
        async _executeJavaScriptInOyaWorld() {
          assert.fail('must not execute');
        },
      },
      '42',
    ),
    /detached/,
  );
});
test('execution failures propagate without retrying or changing worlds', async () => {
  let calls = 0;
  const failure = new Error('document was replaced');
  await assert.rejects(
    evaluateFrame(
      {
        detached: false,
        async _executeJavaScriptInOyaWorld() {
          calls++;
          throw failure;
        },
      },
      '42',
    ),
    (error) => error === failure,
  );
  assert.equal(calls, 1);
});
