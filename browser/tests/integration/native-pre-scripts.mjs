/** Launch native pre-script coverage exclusively in the explicitly selected Oya engine. */
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
const executable = process.env.OYA_NATIVE_ENGINE;
if (!executable) throw Error('Set OYA_NATIVE_ENGINE to patched Oya; no browser fallback is allowed');
const profile = await mkdtemp(join(tmpdir(), 'oya-native-pre-scripts-'));
const fixture = fileURLToPath(new URL('./native-pre-scripts-electron.cjs', import.meta.url));
const child = spawn(executable, [fixture], {
  stdio: 'inherit',
  env: { ...process.env, ELECTRON_RUN_AS_NODE: '', OYA_PRE_SCRIPT_PROFILE: profile },
});
child.on('error', (error) => {
  console.error(error);
  process.exitCode = 1;
});
child.on('close', async (code) => {
  process.exitCode = code ?? 1;
  await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
});
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
