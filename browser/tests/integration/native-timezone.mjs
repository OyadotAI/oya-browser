/** Launch the native timezone prerequisite only in an explicit Oya engine, with parent-owned profile cleanup. */
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
const executable = process.env.OYA_NATIVE_ENGINE;
if (!executable) throw Error('Set OYA_NATIVE_ENGINE to the patched Oya executable; no fallback is allowed');
const profile = await mkdtemp(join(tmpdir(), 'oya-native-timezone-'));
const fixture = fileURLToPath(new URL('./native-timezone-electron.cjs', import.meta.url));
const child = spawn(executable, [fixture], {
  stdio: 'inherit',
  env: { ...process.env, ELECTRON_RUN_AS_NODE: '', OYA_TIMEZONE_PROFILE: profile },
});
child.on('error', (error) => {
  console.error(error);
  process.exitCode = 1;
});
child.on('close', async (code) => {
  process.exitCode = code ?? 1;
  try {
    await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  } catch (error) {
    console.error(error);
    process.exitCode = 1;
  }
});
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
