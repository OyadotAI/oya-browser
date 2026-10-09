/** Run the isolated-world security regression only against an explicitly supplied patched Oya engine. */
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const executable = process.env.OYA_NATIVE_ENGINE;
if (!executable)
  throw new Error('Set OYA_NATIVE_ENGINE to the patched Oya executable; no stock-engine fallback is allowed');
const fixture = fileURLToPath(new URL('./native-runtime-worlds-electron.cjs', import.meta.url));
const child = spawn(executable, [fixture, ...process.argv.slice(2)], { stdio: 'inherit' });
child.on('error', (error) => {
  console.error(error);
  process.exitCode = 1;
});
child.on('exit', (code) => {
  process.exitCode = code ?? 1;
});
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
