/** Launch only Oya's runtime for the real-window regression, with no browser automation provider. */
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const executable = process.env.OYA_NATIVE_ENGINE;
if (!executable)
  throw Error('Set OYA_NATIVE_ENGINE to the patched Oya executable; no stock-engine fallback is allowed');
if (process.env.OYA_WINDOWS_PERSONA_TEST)
  throw Error('Persona window coverage requires native protection; the legacy CDP fixture is not allowed');
const fixture = fileURLToPath(new URL('./windows-electron.cjs', import.meta.url));
const child = spawn(executable, [fixture, ...process.argv.slice(2)], { stdio: 'inherit' });
child.on('error', (error) => {
  console.error(error);
  process.exitCode = 1;
});
child.on('exit', (code) => {
  process.exitCode = code ?? 1;
});
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
