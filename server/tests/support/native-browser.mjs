/** Private stdio fixture bridge to the patched Oya engine; no CDP or substitute browser provider. */
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';
/** Bound engine startup and every request instead of allowing a failed fixture to hang CI. */
const DEADLINE_MS = 30000;
/** A wedged fixture must not prevent CI shutdown or profile cleanup. */
const CLOSE_GRACE_MS = 2000;
/** An explicit engine is mandatory; missing installation is a failure, never a skipped assertion. */
export async function openNativeFixture() {
  const engine = process.env.OYA_NATIVE_ENGINE;
  if (!engine) throw Error('Set OYA_NATIVE_ENGINE to the patched Oya executable; no substitute browser is allowed');
  const profile = mkdtempSync(join(tmpdir(), 'oya-server-native-'));
  const harness = fileURLToPath(
    new URL('../../../browser/tests/integration/server-fixture-electron.cjs', import.meta.url),
  );
  const child = spawn(engine, [harness], {
    stdio: ['pipe', 'pipe', 'pipe'],
    env: { ...process.env, ELECTRON_RUN_AS_NODE: '', OYA_SERVER_FIXTURE_PROFILE: profile },
  });
  return connect(child, profile);
}
/** Match private pipe replies to requests, without publishing a listening endpoint. */
async function connect(child, profile) {
  const pending = new Map();
  let sequence = 0;
  let closed = false;
  const lines = createInterface({ input: child.stdout });
  child.stderr.on('data', (data) => process.stderr.write(data));
  const stopped = new Promise((resolve) => child.once('close', resolve));
  const fail = (error) => {
    for (const p of pending.values()) {
      clearTimeout(p.timer);
      p.reject(error);
    }
    pending.clear();
  };
  child.on('error', fail);
  child.stdin.on('error', fail);
  child.on('close', () => {
    closed = true;
    fail(Error('Native Oya fixture exited'));
  });
  lines.on('line', (line) => receive(line, pending));
  const request = (method, params = {}) =>
    new Promise((resolve, reject) => {
      if (closed) return reject(Error('Native Oya fixture is closed'));
      const id = ++sequence;
      const timer = setTimeout(() => {
        pending.delete(id);
        child.kill();
        reject(Error('Native fixture request timed out: ' + method));
      }, DEADLINE_MS);
      pending.set(id, { resolve, reject, timer });
      child.stdin.write(JSON.stringify({ id, method, params }) + '\n', (error) => {
        if (error) fail(error);
      });
    });
  const close = async () => {
    closed = true;
    child.kill();
    const force = setTimeout(() => child.kill('SIGKILL'), CLOSE_GRACE_MS);
    try {
      await stopped;
    } finally {
      clearTimeout(force);
      lines.close();
    }
    rmSync(profile, { recursive: true, force: true });
  };
  try {
    await request('ready');
  } catch (error) {
    await close();
    throw error;
  }
  return {
    evaluateMain: (expression) => request('evaluate', { expression }),
    evaluate: (expression) => request('evaluate', { expression }),
    send: (action, params) => request('send', { action, params }),
    prepareProfile: (auth) => request('profile_init', auth),
    preparePersona: (auth) => request('profile_init', { ...auth, nativePersona: true }),
    captureProfile: (id) => request('profile_capture', { id }),
    cookies: () => request('profile_cookies'),
    frontDoor: () => request('native_front_door'),
    close,
  };
}
/** Ignore ordinary engine stdout, but reject malformed fixture responses rather than guessing success. */
function receive(line, pending) {
  if (!line.startsWith('OYA_FIXTURE ')) return;
  const reply = JSON.parse(line.slice('OYA_FIXTURE '.length));
  const task = pending.get(reply.id);
  if (!task) return;
  pending.delete(reply.id);
  clearTimeout(task.timer);
  if (reply.error) task.reject(Error(reply.error));
  else task.resolve(reply.value);
}
