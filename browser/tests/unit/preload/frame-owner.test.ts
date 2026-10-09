/** Only bounded selectors reach the native owner lookup; unsupported engines fail closed. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { frameOwnerToken } from '../../../src/preload/frame-owner.ts';
import { NATIVE_RECORDING } from '../../../src/shared/native-recording.ts';
test('uses the native lookup without altering selectors or guessing identities', () => {
  const frame = {
    _oyaFrameTokenForSelector(selector: string) {
      assert.equal(this, frame);
      assert.equal(selector, 'iframe[id=child]');
      return 'native-token';
    },
  };
  assert.equal(frameOwnerToken(frame, 'iframe[id=child]'), 'native-token');
});
test('rejects invalid and unbounded selectors before calling the native API', () => {
  const frame = {
    _oyaFrameTokenForSelector() {
      assert.fail('Invalid selector must not reach native code');
    },
  };
  for (const selector of ['', null, {}, 'x'.repeat(NATIVE_RECORDING.MAX_SELECTOR_CHARS + 1)]) {
    assert.throws(() => frameOwnerToken(frame, selector), /Invalid/);
  }
});
test('missing engine capability is explicit rather than a DOM fallback', () => {
  assert.throws(() => frameOwnerToken({}, 'iframe'), /unsupported/);
});
