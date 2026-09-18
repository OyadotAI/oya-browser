/**
 * Desktop human-control handoff: acquire waits for in-flight commands, only the holder
 * may act, renew/expiry/return behave, and the same gates hold over a real browser
 * WebSocket, where a disconnect drains the desktop's local command slots.
 */
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { WebSocket, WebSocketServer } from 'ws';
const directory = await mkdtemp(join(tmpdir(), 'oya-desktop-control-'));
Object.assign(process.env, {
  OYA_DATA_DIR: directory,
  OYA_PROFILE_SECRET: 'desktop-contract',
  API_KEYS: 'desktop-owner',
});
delete process.env.SUPABASE_URL;
delete process.env.SUPABASE_SERVICE_KEY;
const { control } = await import('../../src/modules/control/service.ts');
const { desktopControl, desktopHolder } = await import('../../src/modules/control/desktop.ts');
const { handleConnection } = await import('../../src/modules/browsers/socket.ts');
const service = control(),
  key = 'desktop-owner',
  id = randomUUID();
const wss = new WebSocketServer({ port: 0, host: '127.0.0.1' });
wss.on('connection', handleConnection);
await new Promise((resolve) => wss.once('listening', resolve));
let client;
try {
  await service.adopt(key, { id, provider: 'oya-desktop' });
  const end = await service.beginCommand(id);
  const transfer = desktopControl(key, id, 'acquire');
  // Wait for the transactional admission gate to close.
  while ((await service.store.get('session', id)).control.mode === 'agent')
    await new Promise((resolve) => setTimeout(resolve, 5));
  await assert.rejects(service.beginCommand(id), { code: 'control_paused' });
  await assert.rejects(service.takeover(key, id, 'acquire', 'other'), { code: 'control_busy' });
  await end();
  assert.equal((await transfer).mine, true);
  await assert.rejects(service.beginCommand(id), { code: 'control_paused' });
  const humanCommand = await service.beginCommand(id, desktopHolder(id));
  await humanCommand();
  assert.equal((await desktopControl(key, id, 'renew')).mode, 'human');
  await assert.rejects(service.takeover(key, id, 'return', 'other'), { code: 'control_busy' });
  assert.equal((await desktopControl(key, id, 'return')).mode, 'agent');
  const resumed = await service.beginCommand(id);
  await resumed();
  await service.takeover(key, id, 'acquire', 'other');
  await assert.rejects(desktopControl(key, id, 'acquire'), { code: 'control_busy' });
  await service.takeover(key, id, 'release', 'other');
  assert.equal(
    (await desktopControl(key, id, 'return')).mode,
    'agent',
    'expired/released leases can explicitly resume',
  );
  await desktopControl(key, id, 'acquire');
  await service.store.transact(async (tx) => {
    (await tx.get('session', id)).control.expiresAt = Date.now() - 1;
  });
  await assert.rejects(desktopControl(key, id, 'renew'), { code: 'control_busy' });
  await assert.rejects(service.beginCommand(id, desktopHolder(id)), { code: 'control_paused' });
  await desktopControl(key, id, 'return');

  client = new WebSocket(`ws://127.0.0.1:${wss.address().port}`);
  const messages = [],
    wait = (predicate) =>
      new Promise((resolve, reject) => {
        const started = Date.now();
        const tick = () => {
          const index = messages.findIndex(predicate);
          if (index >= 0) return resolve(messages.splice(index, 1)[0]);
          if (Date.now() - started > 12000) return reject(new Error('Timed out waiting for control response'));
          setTimeout(tick, 5);
        };
        tick();
      });
  client.on('message', (raw) => messages.push(JSON.parse(raw)));
  await new Promise((resolve) => client.once('open', resolve));
  client.send(JSON.stringify({ type: 'auth', api_key: key, browser_id: id, browser_name: 'Desktop test' }));
  assert.equal((await wait((m) => m.type === 'auth_ok')).control.mode, 'agent');
  const request = async (action, extra = {}) => {
    const requestId = randomUUID();
    client.send(JSON.stringify({ type: 'desktop_control', id: requestId, action, ...extra }));
    return wait((m) => m.id === requestId);
  };
  const command = await request('command-start');
  assert(command.token);
  const taking = request('acquire');
  while ((await service.store.get('session', id)).control.mode === 'agent')
    await new Promise((resolve) => setTimeout(resolve, 5));
  assert((await request('command-start')).error, 'local CDP admission stops during handoff');
  await request('command-end', { token: command.token });
  assert.equal((await taking).state.mode, 'human');
  assert.equal((await request('return')).state.mode, 'agent');
  const outstanding = await request('command-start');
  assert(outstanding.token);
  client.close();
  await new Promise((resolve) => client.once('close', resolve));
  await new Promise((resolve) => setTimeout(resolve, 30));
  assert.equal((await service.store.get('session', id)).inFlight, 0, 'disconnect drains local command slots');
  console.log(
    'Desktop control passed: admission drain, ownership, renewal, expiry, return, real WebSocket handoff and local CDP gates.',
  );
} finally {
  client?.terminate();
  for (const ws of wss.clients) ws.terminate();
  await new Promise((resolve) => wss.close(resolve));
  await service.store.close();
  await rm(directory, { recursive: true, force: true });
}
