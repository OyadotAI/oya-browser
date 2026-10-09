/** Run recorder behavior only in the explicitly selected patched Oya engine. */
import { spawn } from 'node:child_process';
const engine = process.env.OYA_NATIVE_ENGINE;
if (!engine) throw Error('Set OYA_NATIVE_ENGINE to the patched Oya executable; no substitute browser is allowed');
const child = spawn(engine, ['tests/integration/recorder-dom-electron.cjs'], { stdio: 'inherit' });
child.on('error', (error) => {
  console.error(error);
  process.exitCode = 1;
});
child.on('exit', (code) => {
  process.exitCode = code ?? 1;
});
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
