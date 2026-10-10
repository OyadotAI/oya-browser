/** Browser imports cannot launch a competitor or attach an internal debugging transport. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { requireOfflineSource } from '../../../../src/main/mirror/capture.ts';

it('Chromium profile import explicitly refuses unsupported native cookie decryption', () => {
  assert.throws(
    () => requireOfflineSource({ kind: 'chromium', name: 'Chrome' } as any),
    /native cookie decryption is unsupported/,
  );
});
it('offline Firefox import remains available without launching a browser', () => {
  assert.doesNotThrow(() => requireOfflineSource({ kind: 'firefox', name: 'Firefox' } as any));
});
