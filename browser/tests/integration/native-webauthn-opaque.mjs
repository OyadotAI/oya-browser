/** Launch native opaque-origin WebAuthn checks only in the explicitly configured patched Oya engine, with disposable state. */
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
const engine = process.env.OYA_NATIVE_ENGINE;
if (!engine) throw Error('Set OYA_NATIVE_ENGINE; no substitute browser is allowed');
const profile = mkdtempSync(join(tmpdir(), 'oya-native-webauthn-opaque-'));
const child = spawn(engine, ['tests/integration/native-webauthn-opaque-electron.cjs'], {
  stdio: 'inherit',
  env: { ...process.env, ELECTRON_RUN_AS_NODE: '', OYA_WEBAUTHN_PROFILE: profile },
});
const deadline = setTimeout(() => child.kill('SIGKILL'), 60000);
child.on('error', (error) => {
  console.error(error.message);
  process.exitCode = 1;
});
child.on('close', (code) => {
  clearTimeout(deadline);
  rmSync(profile, { recursive: true, force: true });
  process.exitCode = code ?? 1;
});
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
