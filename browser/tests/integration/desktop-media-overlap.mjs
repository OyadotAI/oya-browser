/** Run media-overlap checks in branded stock Oya with a disposable profile and fake devices. */
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { developmentExecutable } from '../../src/dev/launch.ts';
const profile = await mkdtemp(join(tmpdir(), 'oya-media-overlap-'));
const fixture = fileURLToPath(new URL('./desktop-media-overlap-electron.cjs', import.meta.url));
const child = spawn(developmentExecutable(), [fixture], {
  stdio: 'inherit',
  env: { ...process.env, OYA_MEDIA_OVERLAP_PROFILE: profile },
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
