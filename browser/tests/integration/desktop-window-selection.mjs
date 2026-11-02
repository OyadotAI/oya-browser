/** Launch only branded Oya with a disposable profile for native picker-window/IPC verification. */
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { developmentExecutable } from '../../src/dev/launch.ts';

const profile = await mkdtemp(join(tmpdir(), 'oya-window-selection-'));
const fixture = fileURLToPath(new URL('./desktop-window-selection-electron.cjs', import.meta.url));
const child = spawn(developmentExecutable(), [fixture], {
  stdio: 'inherit',
  env: { ...process.env, OYA_WINDOW_SELECTION_PROFILE: profile },
});
child.on('error', (error) => {
  console.error(error);
  process.exitCode = 1;
});
child.on('close', async (code) => {
  await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  process.exitCode = code ?? 1;
});
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
