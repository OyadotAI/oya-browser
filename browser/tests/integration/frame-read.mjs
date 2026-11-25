/** Run the cross-origin frame read regression on stock Electron, as the desktop app ships, or on OYA_NATIVE_ENGINE when set. */
import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
const executable = process.env.OYA_NATIVE_ENGINE || createRequire(import.meta.url)('electron');
const child = spawn(executable, ['tests/integration/frame-read-electron.cjs'], { stdio: 'inherit' });
child.on('error', (error) => {
  console.error(error);
  process.exitCode = 1;
});
child.on('exit', (code) => {
  process.exitCode = code ?? 1;
});
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
