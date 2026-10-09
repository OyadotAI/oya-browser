/** Launch only Oya's runtime for the real-window regression, with no browser automation provider. */
import { spawn } from 'node:child_process';
import { developmentExecutable } from '../../src/dev/launch.ts';
const child = spawn(developmentExecutable(), ['tests/integration/windows-electron.cjs'], { stdio: 'inherit' });
child.on('error', (error) => {
  console.error(error);
  process.exitCode = 1;
});
child.on('exit', (code) => {
  process.exitCode = code ?? 1;
});
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
