/** Native output validation preserves browser-owned attribution without trusting payload paths. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { nativeRecordingOutput } from '../../../../src/main/recording/native-output.ts';
import type { NativeRecordingDocument } from '../../../../src/main/native/index.ts';
/** Authorization-time paths are immutable and independent of renderer output. */
const document = { frames: Object.freeze(['iframe#owned']) } as NativeRecordingDocument;
test('replaces payload frame paths without mutating the payload or authorized document', () => {
  const input = { steps: [{ action: 'type', frames: ['iframe#forged'], text: 'query' }], secrets: ['password'] };
  const output = nativeRecordingOutput(input, document);
  assert.deepEqual(output.steps?.[0].frames, ['iframe#owned']);
  assert.deepEqual(input.steps[0].frames, ['iframe#forged']);
  assert.notEqual(output.steps?.[0].frames, document.frames);
  assert.deepEqual(output.secrets, ['password']);
});
test('rejects malformed steps and secret descriptors rather than coercing them', () => {
  for (const value of [
    null,
    [],
    'text',
    { steps: {} },
    { steps: [null] },
    { steps: [[]] },
    { steps: [{ action: 3 }] },
    { secrets: {} },
    { secrets: [null] },
    { secrets: [{ name: 'password' }] },
  ]) {
    assert.throws(() => nativeRecordingOutput(value, document), /Invalid native recording/);
  }
});
test('accepts empty batches and rejects no valid secret-only batch', () => {
  assert.deepEqual(nativeRecordingOutput({}, document), { steps: undefined });
  assert.deepEqual(nativeRecordingOutput({ secrets: ['password'] }, document), {
    steps: undefined,
    secrets: ['password'],
  });
});
