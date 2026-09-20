/**
 * Remove a scratch directory a browser was using.
 *
 * Chrome's children — the zygote, renderers, the crashpad handler — are not in
 * the test's process tree and keep writing to the profile for a moment after
 * the parent exits. The directory refills while rmSync is walking it and the
 * call throws ENOTEMPTY.
 *
 * Every test that launches a browser had its own guess at a retry budget: 5
 * attempts here, 10 there, and in test-gateway.js none at all. That last one
 * is what turned a green suite red on CI, where the runner is slower than a
 * laptop and the children linger longer.
 *
 * A directory left behind in the OS temp dir is not worth failing a run over.
 * Retry for a few seconds, then say so and carry on.
 */

import { rmSync } from 'node:fs';

export function removeScratch(dir) {
  if (!dir) return;
  try {
    rmSync(dir, { recursive: true, force: true, maxRetries: 30, retryDelay: 200 });
  } catch (e) {
    console.warn(`  ⚠️  left ${dir} behind (${e.code}); a browser process was still writing to it`);
  }
}

// Self-check: node test-support/scratch.js
// The whole point is the failure path, so make it fail. A directory inside a
// read-only parent cannot be unlinked, which is the same "rmSync throws" shape
// as a profile Chrome is still writing to — and it must not take the run down.
if (import.meta.url === `file://${process.argv[1]}`) {
  const { mkdtempSync, mkdirSync, chmodSync, existsSync } = await import('node:fs');
  const { join } = await import('node:path');
  const { tmpdir } = await import('node:os');
  const assert = (await import('node:assert/strict')).default;

  const gone = mkdtempSync(join(tmpdir(), 'oya-scratch-ok-'));
  removeScratch(gone);
  assert.equal(existsSync(gone), false, 'a removable directory is removed');

  const parent = mkdtempSync(join(tmpdir(), 'oya-scratch-stuck-'));
  const child = join(parent, 'locked');
  mkdirSync(child);
  chmodSync(parent, 0o500); // no write: the unlink fails
  removeScratch(child); // must warn, must not throw
  chmodSync(parent, 0o700);
  removeScratch(parent);

  removeScratch(join(tmpdir(), 'oya-scratch-never-existed'));
  removeScratch(undefined);
  console.log('ok — removeScratch removes what it can and never throws');
}
