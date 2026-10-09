/** Run compatibility regression only in an explicitly supplied patched Oya runtime. */
import { spawn } from 'node:child_process';
const executable = process.env.OYA_NATIVE_ENGINE;
if (!executable)
  throw new Error('Set OYA_NATIVE_ENGINE to the patched Oya executable; no substitute engine is allowed');
const child = spawn(executable, ['tests/integration/native-front-door-electron.cjs'], { stdio: 'inherit' });
child.on('error', (error) => {
  console.error(error);
  process.exitCode = 1;
});
child.on('exit', (code) => {
  process.exitCode = code ?? 1;
});
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
