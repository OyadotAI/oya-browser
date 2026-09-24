/**
 * A data directory of the test file's own. Modules that keep state on disk
 * (storage, the control store) pick their paths when
 * they load, so a test file calls ownDataDir() first and then imports them
 * dynamically: it never shares a file with another suite running in parallel.
 */
import { after } from 'node:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

/** Points OYA_DATA_DIR at a fresh directory, with a fixed sealing secret, and removes it after the file's tests. */
export function ownDataDir(prefix = 'oya-unit-') {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  const saved = { dir: process.env.OYA_DATA_DIR, secret: process.env.OYA_PROFILE_SECRET };
  process.env.OYA_DATA_DIR = dir;
  // A set secret means nothing generates (and warns about) a .secret file.
  process.env.OYA_PROFILE_SECRET ||= 'unit-test-secret';
  after(async () => {
    // Storage holds its file open in this directory; close it before the directory goes.
    await (await import('../../../src/platform/storage/index.ts')).closeConnection();
    restoreEnv('OYA_DATA_DIR', saved.dir);
    restoreEnv('OYA_PROFILE_SECRET', saved.secret);
    rmSync(dir, { recursive: true, force: true });
  });
  return dir;
}

/** Puts an environment variable back as it was, deleting it when it was unset. */
export function restoreEnv(name: string, value: string | undefined) {
  if (value === undefined) delete process.env[name];
  else process.env[name] = value;
}
