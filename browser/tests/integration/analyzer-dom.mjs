/** Launch only Oya's runtime for the analyzer DOM regression, with no browser automation provider. */
import { spawn } from 'node:child_process';
import { developmentExecutable } from '../../src/dev/launch.ts';
// Only the hermetic local-file fixture uses this ephemeral Linux CI sandbox exception.
const ciArgs = process.platform === 'linux' && process.env.CI === 'true' ? ['--no-sandbox'] : [];
const child = spawn(developmentExecutable(), ['tests/integration/analyzer-dom-electron.cjs', ...ciArgs], {
  stdio: 'inherit',
});
child.on('error', (error) => {
  console.error(error);
  process.exitCode = 1;
});
child.on('exit', (code) => {
  process.exitCode = code ?? 1;
});
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
