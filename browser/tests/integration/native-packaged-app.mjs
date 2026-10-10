/** Test the shipped Oya executable with a fresh profile through its authenticated native compatibility boundary. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, isAbsolute } from 'node:path';
import { createServer } from 'node:http';
import { randomBytes } from 'node:crypto';
import { once } from 'node:events';
import { WebSocket } from 'ws';
const executable = process.env.OYA_PACKAGED_EXECUTABLE;
assert.ok(executable && isAbsolute(executable), 'Set OYA_PACKAGED_EXECUTABLE to the packaged Oya executable');
const profile = await mkdtemp(join(tmpdir(), 'oya-packaged-test-'));
const token = randomBytes(32).toString('hex');
const site = createServer((_req, res) => {
  res.setHeader('Content-Type', 'text/html');
  res.end(
    '<!doctype html><title>Packaged Oya</title><input id=field><script>globalThis.trusted=[];document.querySelector("input").oninput=e=>trusted.push(e.isTrusted)</script>',
  );
});
await new Promise((resolve) => site.listen(0, '127.0.0.1', resolve));
const reservation = createServer();
await new Promise((resolve) => reservation.listen(0, '127.0.0.1', resolve));
const port = reservation.address().port;
await new Promise((resolve) => reservation.close(resolve));
await writeFile(join(profile, 'config.json'), JSON.stringify({ ui: { importOffered: true } }));
const url = `http://127.0.0.1:${site.address().port}/`;
const child = spawn(executable, [url], {
  stdio: ['ignore', 'inherit', 'inherit'],
  env: {
    ...process.env,
    OYA_USER_DATA_DIR: profile,
    OYA_AUTO_CONNECT: 'false',
    OYA_NATIVE_CDP_PORT: String(port),
    OYA_NATIVE_CDP_TOKEN: token,
  },
});
let socket;
const timeout = setTimeout(() => child.kill('SIGKILL'), 60000);
/** Wait only for our authenticated listener and fail if this candidate exits. */
async function endpoint() {
  for (let i = 0; i < 250; i++) {
    if (child.exitCode !== null) throw Error(`Packaged Oya exited ${child.exitCode}`);
    const response = await fetch(`http://127.0.0.1:${port}/json/version`, {
      headers: { authorization: `Bearer ${token}` },
    }).catch(() => null);
    if (response?.ok) {
      const data = await response.json();
      assert.equal(data.Browser, 'Oya/native');
      return data.webSocketDebuggerUrl;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw Error('Packaged native listener did not start');
}
try {
  socket = new WebSocket(await endpoint(), { headers: { authorization: `Bearer ${token}` } });
  await once(socket, 'open');
  const pending = new Map();
  let sequence = 0;
  socket.on('message', (raw) => {
    const message = JSON.parse(raw.toString()),
      waiter = pending.get(message.id);
    if (!waiter) return;
    pending.delete(message.id);
    message.error ? waiter.reject(Error(message.error.message)) : waiter.resolve(message.result);
  });
  const call = (method, params = {}, sessionId) =>
    new Promise((resolve, reject) => {
      const id = ++sequence;
      pending.set(id, { resolve, reject });
      socket.send(JSON.stringify({ id, method, params, sessionId }));
    });
  const { targetId } = await call('Target.createTarget', { url });
  const { sessionId } = await call('Target.attachToTarget', { targetId, flatten: true });
  await call('Runtime.evaluate', { expression: 'document.querySelector("input").focus()' }, sessionId);
  await call('Input.insertText', { text: 'Packaged native 日本語' }, sessionId);
  const reply = await call(
    'Runtime.evaluate',
    {
      expression: '[document.querySelector("input").value,trusted.length>0&&trusted.every(Boolean)]',
      returnByValue: true,
    },
    sessionId,
  );
  assert.deepEqual(reply.result.value, ['Packaged native 日本語', true]);
  await call('Page.bringToFront', {}, sessionId);
  const shot = await call('Page.captureScreenshot', {}, sessionId);
  assert.ok(Buffer.from(shot.data, 'base64').length > 1000);
  await call('Target.closeTarget', { targetId });
  console.log('PASS packaged Oya: protected tab, authenticated native input, screenshot and target cleanup');
} finally {
  socket?.close();
  clearTimeout(timeout);
  const exited = child.exitCode === null ? once(child, 'exit') : Promise.resolve();
  child.kill('SIGTERM');
  await exited;
  await new Promise((resolve) => site.close(resolve));
  await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}
