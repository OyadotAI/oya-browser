/** Real Docker/Electron contract. Requires a locally built governance-enabled image. */
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { randomUUID } from 'node:crypto';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createServer } from 'node:http';
import express from 'express';
import { WebSocketServer } from 'ws';
const exec = promisify(execFile);
const docker = async (args) =>
  (await exec('docker', args, { timeout: 120000, maxBuffer: 2 * 1024 * 1024 })).stdout.trim();
const tag = randomUUID().slice(0, 8),
  network = `oya-test-${tag}`,
  relay = `oya-relay-${tag}`;
const data = await mkdtemp(join(tmpdir(), 'oya-managed-test-'));
Object.assign(process.env, {
  OYA_DATA_DIR: data,
  API_KEYS: 'managed-test',
  OYA_PROFILE_SECRET: 'managed-test-secret',
  OYA_MANAGED_NETWORK: network,
  OYA_MANAGED_IMAGE: process.env.OYA_TEST_IMAGE || 'oya-browser:control-test',
  OYA_MANAGED_REGION: 'local',
  OYA_MANAGED_CONTROL_URL: `ws://${relay}:3100/ws`,
  OYA_MANAGED_PROXY_URL: `http://${relay}:8899`,
});
delete process.env.SUPABASE_URL;
delete process.env.SUPABASE_SERVICE_KEY;
const { router } = await import('../../src/app/api.ts');
const { createEgressServer } = await import('../../src/modules/control/egress.ts');
const { handleConnection } = await import('../../src/modules/browsers/socket.ts');
const { control } = await import('../../src/modules/control/service.ts');
const { registry } = await import('../../src/modules/browsers/registry.ts');
const { removeManaged } = await import('../../src/modules/control/managed.ts');
const app = express();
app.use(express.json());
app.use('/api', router);
const server = createServer(app),
  egress = createEgressServer(),
  wss = new WebSocketServer({ noServer: true });
server.on('upgrade', (req, socket, head) => wss.handleUpgrade(req, socket, head, (ws) => handleConnection(ws, req)));
const listen = (server) => new Promise((resolve) => server.listen(0, '0.0.0.0', resolve));
await listen(server);
await listen(egress);
const base = `http://127.0.0.1:${server.address().port}`;
const call = async (path, body, method = 'POST') => {
  const response = await fetch(`${base}/api${path}`, {
    method,
    headers: { Authorization: 'Bearer managed-test', 'Content-Type': 'application/json' },
    ...(body ? { body: JSON.stringify(body) } : {}),
  });
  return { status: response.status, body: await response.json() };
};
let id;
try {
  await docker(['network', 'create', '--internal', network]);
  const code = `const net=require('net'); for(const [port,target] of [[3100,${server.address().port}],[8899,${egress.address().port}]]) net.createServer(s=>{const u=net.connect(target,'host.docker.internal');s.pipe(u).pipe(s);s.on('error',()=>u.destroy());u.on('error',()=>s.destroy());s.on('close',()=>u.destroy());}).listen(port,'0.0.0.0')`;
  await docker([
    'run',
    '-d',
    '--name',
    relay,
    '--network',
    network,
    '--cap-drop',
    'ALL',
    '--entrypoint',
    'node',
    process.env.OYA_MANAGED_IMAGE,
    '-e',
    code,
  ]);
  await docker(['network', 'connect', 'bridge', relay]);
  const started = await call('/browsers/start', {
    provider: 'oya-selfhosted',
    governed: true,
    policy: { allowedHosts: ['example.com'], region: 'local' },
  });
  assert.equal(started.status, 201, JSON.stringify(started.body));
  id = started.body.id;
  const deadline = Date.now() + 90000;
  while (!registry.has?.(id) && !registry.get(id) && Date.now() < deadline)
    await new Promise((r) => setTimeout(r, 500));
  assert.ok(registry.get(id), 'real Electron browser enrolled');
  const allowed = await call(`/browsers/${id}/command`, { action: 'navigate', params: { url: 'https://example.com' } });
  assert.equal(allowed.body.ok, true, JSON.stringify(allowed));
  const denied = await call(`/browsers/${id}/command`, { action: 'navigate', params: { url: 'https://example.org' } });
  assert.equal(denied.body.ok, false, 'browser hook blocks a disallowed destination');
  const direct = await docker([
    'exec',
    `oya-managed-${id}`,
    'node',
    '-e',
    "const s=require('net').connect(443,'1.1.1.1');s.setTimeout(2000);s.on('connect',()=>process.exit(2));s.on('error',()=>process.exit(0));s.on('timeout',()=>process.exit(0))",
  ]);
  assert.equal(direct, '', 'container cannot bypass proxy with a direct public connection');
  const takeover = await call(`/control/sessions/${id}/control`, { action: 'acquire' });
  assert.equal(takeover.status, 200);
  assert.equal(
    (await call(`/browsers/${id}/command`, { action: 'navigate', params: { url: 'https://example.com' } })).status,
    409,
  );
  await call(`/control/sessions/${id}/control`, { action: 'release' });
  assert.equal((await control().read('managed-test')).sessions.find((s) => s.id === id).control.mode, 'paused');
  await removeManaged(`oya-managed-${id}`, 'managed-test', id);
  await control().update('managed-test', id, { state: 'stopped' });
  console.log(
    'Managed runtime passed: internal network, real Electron enrollment, allowed navigation, denied destination, direct-egress isolation, takeover, resource removal.',
  );
} finally {
  if (id) await removeManaged(`oya-managed-${id}`, 'managed-test', id).catch(() => {});
  await docker(['rm', '-f', relay]).catch(() => {});
  await docker(['network', 'rm', network]).catch(() => {});
  for (const ws of wss.clients) ws.terminate();
  server.closeAllConnections();
  egress.closeAllConnections();
  server.close();
  egress.close();
  wss.close();
  await control().store.close();
  await rm(data, { recursive: true, force: true });
}
process.exit(0);
