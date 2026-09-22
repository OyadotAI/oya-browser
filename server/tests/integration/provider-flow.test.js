/**
 * Saved gateway providers through the real API: validation, per-key isolation
 * and encrypted persistence, the caller's own saved credentials on connect,
 * start and the gateway, releasing a failed hosted connection before failover,
 * and retryable failed stops.
 */
import assert from 'node:assert/strict';
import express from 'express';
import { WebSocket, WebSocketServer } from 'ws';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const data = await mkdtemp(join(tmpdir(), 'oya-provider-flow-'));
process.env.OYA_DATA_DIR = data;
process.env.API_KEYS = 'owner-a,owner-b';
process.env.OYA_PROFILE_SECRET = 'test-provider-flow-secret';
process.env.OYA_ALLOW_PRIVATE_TARGETS = 'true';
delete process.env.SUPABASE_URL;
delete process.env.SUPABASE_SERVICE_KEY;
for (const key of ['STEEL_API_KEY', 'ANCHOR_API_KEY', 'BROWSERBASE_API_KEY', 'BROWSERUSE_API_KEY', 'OYA_PROVIDERS'])
  delete process.env[key];
const { router } = await import('../../src/app/api.ts');
const { handleUpgrade, sessions } = await import('../../src/modules/gateway/service.ts');
const { registry } = await import('../../src/modules/browsers/registry.ts');
const { pool, ProviderPool } = await import('../../src/modules/gateway/routing.ts');
const config = await import('../../src/modules/config/service.ts');
const { fingerprint } = await import('../../src/platform/audit.ts');
const app = express();
app.use(express.json());
app.use(router);
const server = app.listen(0, '127.0.0.1');
await new Promise((r) => server.once('listening', r));
const origin = `http://127.0.0.1:${server.address().port}`;
const upstreamServer = new WebSocketServer({ noServer: true });
server.on('upgrade', (req, socket, head) => {
  if (req.url === '/upstream')
    upstreamServer.handleUpgrade(req, socket, head, (ws) => upstreamServer.emit('connection', ws, req));
  else handleUpgrade(req, socket, head).catch(() => socket.destroy());
});
const realFetch = globalThis.fetch;
const request = async (path, body, method = 'POST', owner = 'owner-a') => {
  const res = await realFetch(origin + path, {
    method,
    headers: { Authorization: `Bearer ${owner}`, 'Content-Type': 'application/json' },
    ...(body ? { body: JSON.stringify(body) } : {}),
  });
  return { status: res.status, body: await res.json() };
};
try {
  for (const cfg of [
    { name: ' ' },
    { name: 'bad/name' },
    { name: 'no-url', type: 'cdp' },
    { name: 'unknown', type: 'made-up' },
    { name: 'zero', type: 'steel', maxConcurrent: 0 },
    { name: 'negative', type: 'steel', weight: -1 },
    { name: 'nan', type: 'steel', priority: 'abc' },
  ])
    assert.equal((await request('/gateway/providers', cfg)).status, 400);
  assert.equal((await request('/gateway/providers', { name: 'steel', type: 'steel' })).status, 409);
  const added = await request('/gateway/providers', {
    name: 'my steel',
    type: 'steel',
    apiKey: 'credential-a',
    priority: 0,
    active: 999,
    cooldownUntil: 9999999999999,
  });
  assert.equal(added.status, 200);
  assert.equal(added.body.priority, 0);
  assert.equal(added.body.active, 0);
  assert.equal(added.body.healthy, true);
  assert(!JSON.stringify(added.body).includes('credential-a'));
  assert.equal(config.envFor('owner-a').STEEL_API_KEY, 'credential-a');
  assert.equal(config.envFor('owner-b').STEEL_API_KEY, undefined);
  assert.equal((await request('/gateway/providers', { name: 'my steel', type: 'steel' })).status, 409);
  const other = await request('/gateway/providers', null, 'GET', 'owner-b');
  assert.equal(other.body.providers.length, 0);
  assert.equal((await request('/gateway/providers/my%20steel', null, 'DELETE', 'owner-b')).status, 404);
  const route = pool.get(fingerprint('owner-a'), 'my steel');
  route.active = 1;
  assert.equal((await request('/gateway/providers/my%20steel', null, 'DELETE')).status, 409);
  route.active = 0;
  await request('/gateway/strategy', { strategy: 'weighted' });
  await request('/gateway/providers', {
    name: 'direct',
    type: 'cdp',
    wsUrl: 'ws://127.0.0.1:9222/cdp?token=private-route-secret',
  });
  await config.drain();
  const disk = await readFile(join(data, 'key-settings.json'), 'utf8');
  assert(!disk.includes('private-route-secret') && !disk.includes('credential-a'), 'secrets encrypted at rest');
  config.reset();
  await config.restore();
  const restored = new ProviderPool();
  config.restoreRouting(restored);
  assert.equal(restored.get(fingerprint('owner-a'), 'my steel').priority, 0);
  assert.equal(restored.strategyFor(fingerprint('owner-a')), 'weighted');
  assert(restored.get(fingerprint('owner-a'), 'direct').wsUrl.includes('private-route-secret'));
  assert.equal(config.get('owner-a')._routing, undefined);
  console.log('✓ add validation, credentials, duplicates, isolation, active removal, encrypted restart persistence');

  // Both browser entry points must use this caller's saved credential.
  for (const path of ['/browsers/connect', '/browsers/start']) {
    let vendorKey;
    globalThis.fetch = async (url, opts) => {
      if (String(url).startsWith('https://api.steel.dev')) {
        vendorKey = opts.headers['steel-api-key'];
        return Response.json({}, { status: 401 });
      }
      return realFetch(url, opts);
    };
    const response = await request(path, { provider: 'steel' });
    assert.equal(response.status, 502);
    assert.equal(vendorKey, 'credential-a');
  }
  // Gateway routes must use the same saved key, and failed handshakes must
  // release the vendor allocation before failing over to another route.
  const upstreamUrl = origin.replace('http:', 'ws:') + '/upstream';
  let failHandshake = false,
    releases = 0;
  globalThis.fetch = async (url, opts) => {
    if (String(url).startsWith('https://api.steel.dev')) {
      assert.equal(opts.headers['steel-api-key'], 'credential-a');
      if (String(url).endsWith('/release')) {
        releases++;
        return Response.json({});
      }
      return Response.json({
        id: 'gateway-vendor',
        websocketUrl: failHandshake ? 'ws://127.0.0.1:1/fail' : upstreamUrl,
      });
    }
    return realFetch(url, opts);
  };
  // Steel puts its key in the connection query string.
  server.removeAllListeners('upgrade');
  server.on('upgrade', (req, socket, head) => {
    if (new URL(req.url, origin).pathname === '/upstream')
      upstreamServer.handleUpgrade(req, socket, head, (ws) => upstreamServer.emit('connection', ws, req));
    else handleUpgrade(req, socket, head).catch(() => socket.destroy());
  });
  pool.setStrategy(fingerprint('owner-a'), 'priority');
  const connect = async () => {
    const client = new WebSocket(origin.replace('http:', 'ws:') + '/connect?token=owner-a');
    await new Promise((resolve, reject) => {
      client.once('open', resolve);
      client.once('error', reject);
    });
    return client;
  };
  const first = await connect();
  assert.equal([...sessions.values()][0].provider, 'my steel');
  await [...sessions.values()][0].destroy('test');
  first.close();
  await new Promise((r) => setTimeout(r, 100));
  assert.equal(releases, 1);
  failHandshake = true;
  pool.register({ name: 'direct', owner: fingerprint('owner-a'), wsUrl: upstreamUrl });
  const fallback = await connect();
  assert.equal([...sessions.values()][0].provider, 'direct');
  assert.equal(releases, 2, 'failed handshake releases hosted allocation');
  await [...sessions.values()][0].destroy('test');
  fallback.close();
  console.log('✓ gateway uses saved credentials and releases failed hosted connections before failover');

  // A failed explicit stop is actionable and leaves the release retryable.
  registry.add('stop-failure', {
    apiKey: 'owner-a',
    clientType: 'cdp',
    provider: 'steel',
    engine: { close() {} },
    release: async () => {
      throw new Error('release failed');
    },
  });
  const stopFailure = await request('/browsers/stop-failure/stop', {});
  assert.equal(stopFailure.status, 502);
  assert.equal(stopFailure.body.ok, false);
  assert(registry.get('stop-failure'));
  assert.equal((await request('/browsers/stop-failure/connection', null, 'DELETE')).status, 502);
  assert(registry.get('stop-failure'));
  registry.get('stop-failure').release = async () => {};
  assert.equal((await request('/browsers/stop-failure/stop', {})).status, 200);
  console.log('✓ failed stops report failure and can be retried');
  globalThis.fetch = realFetch;
  await request('/gateway/providers/my%20steel', null, 'DELETE');
  await config.drain();
  config.reset();
  await config.restore();
  const afterDelete = new ProviderPool();
  config.restoreRouting(afterDelete);
  assert.equal(afterDelete.get(fingerprint('owner-a'), 'my steel'), undefined);
  console.log('✓ connect/start use saved credentials; removed routes stay removed after restart');
} finally {
  globalThis.fetch = realFetch;
  for (const session of [...sessions.values()]) await session.destroy('teardown');
  for (const client of upstreamServer.clients) client.terminate();
  upstreamServer.close();
  await config.drain();
  await new Promise((r) => server.close(r));
  await rm(data, { recursive: true, force: true });
}
process.exit(0);
