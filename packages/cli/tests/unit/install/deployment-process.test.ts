/** Probe deadlines must include credential helpers which retain inherited output pipes. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { deploymentProbe } from '../../../src/install/deployment-process.ts';

describe('deployment probes', () => {
  it('settles at its deadline and removes helpers that retain pipes after the direct child exits', async () => {
    const root = mkdtempSync(join(tmpdir(), 'oya-probe-')),
      path = join(root, 'pid');
    const script = '/bin/sleep 60 & printf "%s" "$!" > "$1"; exit 0';
    let fallback: ReturnType<typeof setTimeout> | undefined;
    try {
      const probe = deploymentProbe('/bin/bash', ['-c', script, 'probe-fixture', path], 100);
      const late = new Promise<string>((resolve) => {
        fallback = setTimeout(() => resolve('deadline missed'), 2000);
      });
      assert.equal(await Promise.race([probe, late]), null);
      const pid = Number(readFileSync(path, 'utf8'));
      await assertHelperGone(pid);
    } finally {
      clearTimeout(fallback);
      try {
        process.kill(Number(readFileSync(path, 'utf8')), 'SIGKILL');
      } catch {
        /* Only our fixture. */
      }
      rmSync(root, { recursive: true, force: true });
    }
  });

  it('refuses unbounded output and returns failed inspections as unavailable', async () => {
    assert.equal(await deploymentProbe(process.execPath, ['-e', 'process.stdout.write("x".repeat(100000))']), null);
    assert.equal(await deploymentProbe('oya-no-such-probe-program', []), null);
  });
});

/** Allow the OS to reap an owned helper which was just terminated. */
async function assertHelperGone(pid: number): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt++) {
    try {
      process.kill(pid, 0);
    } catch {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  assert.fail('probe left an owned credential helper alive');
}
