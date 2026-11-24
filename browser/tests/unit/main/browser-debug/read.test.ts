/** Human-readable diagnostics preserve byte boundaries for international page and URL content. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { debugBytePrefix } from '../../../../src/main/browser-debug/read.ts';
test('UTF-8 output and omission notice remain within the specified byte budget', () => {
  for (const limit of [0, 1, 12, 20, 255, 1024]) {
    const result = debugBytePrefix('页面🌊'.repeat(400), limit);
    assert.ok(Buffer.byteLength(result) <= limit);
    assert.ok(!result.includes('\uFFFD'), 'no split UTF-8 character');
  }
  assert.equal(debugBytePrefix('short', 20), 'short');
});
