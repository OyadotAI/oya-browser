/** A recording preload cannot be used as a general IPC bridge or an unbounded message source. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { sendRecording } from '../../../src/preload/recording-bridge.ts';
import { NATIVE_RECORDING } from '../../../src/shared/native-recording.ts';
test('uses only the fixed channel and returns no capability', () => {
  const sent: unknown[][] = [];
  assert.equal(
    sendRecording((...args) => sent.push(args), 'epoch', '{"steps":[]}'),
    undefined,
  );
  assert.deepEqual(sent, [[NATIVE_RECORDING.CHANNEL, 'epoch', '{"steps":[]}']]);
});
test('refuses non-string and oversized batches before sending', () => {
  const send = () => assert.fail('invalid data must not be sent');
  for (const [epoch, payload] of [
    [null, '{}'],
    ['x'.repeat(NATIVE_RECORDING.MAX_EPOCH_CHARS + 1), '{}'],
    ['', '{}'],
    ['epoch', {}],
    ['epoch', 'x'.repeat(NATIVE_RECORDING.MAX_PAYLOAD_CHARS + 1)],
  ]) {
    assert.throws(() => sendRecording(send, epoch, payload), /Invalid|too large/);
  }
});
