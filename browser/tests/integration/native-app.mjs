/** Launch only Oya's runtime for the native application boot regression, with no browser automation provider. */
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
const executable = process.env.OYA_NATIVE_ENGINE;
if (!executable) throw new Error('Set OYA_NATIVE_ENGINE to the patched Oya executable; no fallback is allowed');
const profile = await mkdtemp(join(tmpdir(), 'oya-native-app-'));
const fixture = fileURLToPath(new URL('./native-app-electron.cjs', import.meta.url));
const child = spawn(executable, [fixture], {
  stdio: 'inherit',
  env: { ...process.env, OYA_NATIVE_APP_PROFILE: profile },
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
    console.error('Native app profile cleanup failed after Electron exited:', error);
    process.exitCode = 1;
  }
});
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
