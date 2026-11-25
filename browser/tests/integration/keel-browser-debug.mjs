/** Launch only the explicit Oya engine and clean its disposable profile after process shutdown. */
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
const engine = process.env.OYA_NATIVE_ENGINE;
if (!engine) throw Error('Explicit Oya runtime required; no substitute browser is allowed');
const profile = mkdtempSync(join(tmpdir(), 'oya-keel-debug-'));
const child = spawn(engine, ['tests/integration/keel-browser-debug.cjs'], {
  stdio: 'inherit',
  env: { ...process.env, OYA_DEBUG_PROFILE: profile },
});
child.on('error', (e) => {
  console.error(e.message);
  rmSync(profile, { recursive: true, force: true });
  process.exitCode = 1;
});
child.on('exit', (code) => {
  rmSync(profile, { recursive: true, force: true });
  process.exitCode = code ?? 1;
});
