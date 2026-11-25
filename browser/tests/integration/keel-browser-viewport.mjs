/** Run the viewport fixture in Oya and remove its disposable profile after native shutdown finishes. */
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
const executable = process.env.OYA_NATIVE_ENGINE;
if (!executable) throw Error('Set OYA_NATIVE_ENGINE to the Oya executable; no substitute browser is allowed');
const profile = await mkdtemp(join(tmpdir(), 'oya-keel-viewport-'));
const fixture = fileURLToPath(new URL('./keel-browser-viewport.cjs', import.meta.url));
const child = spawn(executable, [fixture], {
  stdio: 'inherit',
  env: { ...process.env, OYA_KEEL_VIEWPORT_PROFILE: profile },
});
/** Interrupt only the native process started for this disposable fixture. */
const interrupt = (signal) => child.kill(signal);
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, interrupt);
try {
  process.exitCode = await new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('close', (code) => resolve(code ?? 1));
  });
} finally {
  for (const signal of ['SIGINT', 'SIGTERM']) process.removeListener(signal, interrupt);
  await rm(profile, { recursive: true, force: true });
  console.log(`Removed fixture profile: ${profile}`);
}
