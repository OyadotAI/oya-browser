/** Launch failures and teardown are tested without starting a source browser. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { CaptureProcess } from '../../../../src/main/mirror/capture-process.ts';
import { capturedOrError } from '../../../../src/main/mirror/capture.ts';
/** Child-process seam that records termination. */
function child() {
  const process = new EventEmitter();
  return Object.assign(process, { kill: () => process.emit('exit', 0) });
}
it('reports spawn errors immediately rather than waiting for a port timeout', async () => {
  const process = child();
  const capture = new CaptureProcess(process as any);
  process.emit('error', new Error('ENOENT'));
  await assert.rejects(capture.ready('/missing'), /Could not launch browser: ENOENT/);
});
it('reports an early process exit', async () => {
  const process = child();
  const capture = new CaptureProcess(process as any);
  process.emit('exit', 1);
  await assert.rejects(capture.ready('/missing'), /exited before import completed/);
});
it('waits for process exit before removing its scratch profile', async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-capture-test-'));
  const process = child();
  process.kill = () => {
    assert.equal(fs.existsSync(dir), true);
    return process.emit('exit', 0);
  };
  await new CaptureProcess(process as any).clean(dir);
  assert.equal(fs.existsSync(dir), false);
});
it('times out readiness with actionable retry guidance', async (t) => {
  t.mock.timers.enable({ apis: ['Date', 'setTimeout'] });
  const waiting = new CaptureProcess(child() as any).ready('/missing');
  const assertion = assert.rejects(waiting, /Close the source browser and retry/);
  t.mock.timers.tick(20000);
  await assertion;
});
it('keeps successful captures and distinguishes total failure from an empty import', () => {
  const profiles = [{ profile: 'Default', cookies: [] }] as any;
  assert.equal(capturedOrError(profiles, ['Work: locked']), profiles);
  assert.throws(() => capturedOrError([], ['Default: locked']), /locked/);
  assert.deepEqual(capturedOrError([], []), []);
});
