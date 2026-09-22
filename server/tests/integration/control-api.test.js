/**
 * The control API end to end over HTTP: discovery privacy, viewer and operator role
 * boundaries, credential and ticket revocation, takeover, drain, queued creation and
 * cancellation, stuck-command takeover and managed-browser credential scope.
 */
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import express from 'express';
const directory = await mkdtemp(join(tmpdir(), 'oya-control-api-'));
Object.assign(process.env, {
  API_KEYS: 'owner,other',
  OYA_OPERATOR_TOKEN: 'operator',
  OYA_PROFILE_SECRET: 'test-control-api',
  OYA_DATA_DIR: directory,
  OYA_STUCK_COMMAND_MS: '300',
});
delete process.env.SUPABASE_URL;
delete process.env.SUPABASE_SERVICE_KEY;
const { router } = await import('../../src/app/api.ts');
const { control } = await import('../../src/modules/control/service.ts');
const { registry } = await import('../../src/modules/browsers/registry.ts');
const { handleJsonList, handleUpgrade } = await import('../../src/modules/gateway/service.ts');
const app = express();
app.use(express.json());
app.use('/api', router);
app.get('/json/list', handleJsonList);
const server = await new Promise((resolve) => {
  const s = app.listen(0, '127.0.0.1', () => resolve(s));
});
server.on('upgrade', (req, socket, head) => void handleUpgrade(req, socket, head).catch(() => socket.destroy()));
const base = `http://127.0.0.1:${server.address().port}`;
async function call(path, method = 'GET', body, key = 'owner') {
  const r = await fetch(base + path, {
    method,
    headers: { Authorization: `Bearer ${key}`, 'Content-Type': 'application/json' },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  return { status: r.status, body: await r.json() };
}
try {
  assert.equal((await fetch(base + '/json/list')).status, 401);
  const id = randomUUID();
  await control().adopt('owner', { id, provider: 'cdp' });
  registry.add(id, {
    apiKey: 'owner',
    name: 'Test',
    engine: { wsUrl: 'ws://127.0.0.1:9', send: async () => ({ ok: true }), close() {} },
    provider: 'cdp',
    clientType: 'cdp',
  });
  const viewer = (await call('/api/control/credentials', 'POST', { role: 'viewer' })).body;
  const operator = (await call('/api/control/credentials', 'POST', { role: 'operator' })).body;
  assert.equal((await call('/api/control', 'GET', undefined, viewer.token)).status, 200);
  assert.equal(
    (await call(`/api/browsers/${id}`, 'GET', undefined, viewer.token)).body.cdpUrl,
    undefined,
    'viewers cannot obtain full-access URLs',
  );
  assert.equal((await call(`/api/control/sessions/${id}/ticket`, 'POST', {}, viewer.token)).status, 403);
  assert.equal((await call('/api/control/credentials', 'POST', {}, operator.token)).status, 403);
  assert.equal((await call('/api/config', 'POST', {}, operator.token)).status, 403);
  assert.equal((await call(`/api/control/sessions/${id}`, 'GET', undefined, 'other')).status, 404);
  const ticket = (await call(`/api/control/sessions/${id}/ticket`, 'POST', {}, operator.token)).body.ticket;
  await call(`/api/control/credentials/${operator.id}`, 'DELETE');
  const original = await control().redeem(ticket, id);
  const { authenticateToken } = await import('../../src/modules/auth/service.ts');
  await assert.rejects(authenticateToken(original), { status: 401 });
  assert.equal((await call('/api/control', 'GET', undefined, operator.token)).status, 401);
  assert.equal((await call(`/api/control/sessions/${id}/control`, 'POST', { action: 'acquire' })).status, 200);
  assert.equal(
    (await call(`/api/browsers/${id}/command`, 'POST', { action: 'navigate', params: { url: 'https://example.com' } }))
      .status,
    409,
  );
  assert.equal(
    (await call(`/api/control/sessions/${id}/input`, 'POST', { action: 'click', params: { element_id: 1 } })).status,
    200,
  );
  assert.equal(
    (await call(`/api/control/sessions/${id}/input`, 'POST', { action: 'keyboard_type', params: { text: 'hello' } }))
      .status,
    200,
  );
  await call(`/api/control/sessions/${id}/control`, 'POST', { action: 'release' });
  assert.equal((await call(`/api/browsers/${id}/command`, 'POST', { action: 'navigate' })).status, 409);
  await call(`/api/control/sessions/${id}/control`, 'POST', { action: 'resume' });
  assert.equal((await call(`/api/browsers/${id}/command`, 'POST', { action: 'navigate' })).status, 200);
  await call(`/api/control/sessions/${id}/control`, 'POST', { action: 'acquire' });
  assert.equal((await call(`/api/control/sessions/${id}/record`, 'POST', { mode: 'start' })).status, 200);
  assert.equal((await call(`/api/control/sessions/${id}/record`, 'POST', { mode: 'stop', resume: true })).status, 200);
  assert.equal(
    (await call(`/api/browsers/${id}/command`, 'POST', { action: 'navigate' })).status,
    200,
    'stopping a recording hands control back to automation',
  );
  assert.equal((await call(`/api/control/sessions/${id}/record`, 'POST', { mode: 'status' })).body.recording, false);
  await call('/api/operator/drain', 'POST', { draining: true }, 'operator');
  for (const path of ['/api/browsers/start', '/api/browsers/connect', '/api/browsers/provision'])
    assert.equal((await call(path, 'POST', {})).status, 503);
  await call('/api/operator/drain', 'POST', { draining: false }, 'operator');
  await call('/api/control/project', 'PATCH', { maxConcurrent: 1 });
  const queued = await call('/api/browsers/start', 'POST', { provider: 'cdp', queueMs: 30000 });
  assert.equal(queued.status, 202);
  assert.equal(queued.body.state, 'queued');
  assert.equal((await call(`/api/control/sessions/${queued.body.id}/cancel`, 'POST', {})).body.state, 'stopped');
  const queuedAgain = await call('/api/browsers/start', 'POST', { provider: 'cdp', queueMs: 30000 });
  assert.equal((await call(`/api/control/sessions/${queuedAgain.body.id}/stop`, 'POST', {})).body.status, 'stopped');
  assert.equal((await call(`/api/control/sessions/${queuedAgain.body.id}/stop`, 'POST', {})).body.ok, true);
  assert.equal(
    (await call('/api/control/events')).body.events.some((e) => e.type === 'session.stopped'),
    true,
  );

  // An unanswered CDP command holds takeover only until it is presumed stuck; acquisition waits for that instead of failing.
  await call('/api/control/project', 'PATCH', { maxConcurrent: null });
  const { WebSocketServer, WebSocket } = await import('ws');
  const silent = new WebSocketServer({ port: 0, host: '127.0.0.1' });
  await new Promise((r) => silent.once('listening', r));
  const quiet = randomUUID();
  await control().adopt('owner', { id: quiet, provider: 'cdp' });
  registry.add(quiet, {
    apiKey: 'owner',
    name: 'Quiet',
    engine: { wsUrl: `ws://127.0.0.1:${silent.address().port}`, send: async () => ({ ok: true }), close() {} },
    provider: 'cdp',
    clientType: 'cdp',
  });
  const cdp = new WebSocket(`${base.replace('http:', 'ws:')}/connect?browser=${quiet}`, {
    headers: { Authorization: 'Bearer owner' },
  });
  await new Promise((resolve, reject) => {
    cdp.once('open', resolve);
    cdp.once('error', reject);
  });
  cdp.send(
    JSON.stringify({
      id: 1,
      method: 'Runtime.evaluate',
      params: { expression: 'new Promise(() => {})', awaitPromise: true },
    }),
  );
  await new Promise((r) => setTimeout(r, 100));
  assert.equal((await control().session('owner', quiet)).inFlight, 1, 'the command holds an in-flight slot');
  assert.equal(
    (await call(`/api/control/sessions/${quiet}/control`, 'POST', { action: 'acquire' })).status,
    200,
    'takeover waits out an unanswered command',
  );
  cdp.terminate();
  silent.close();
  // Managed browsers get a credential bound to their session instead of the project key.
  const enrolled = await control().enrollmentCredential('owner', quiet);
  assert.equal(
    (await call('/api/control', 'GET', undefined, enrolled.token)).status,
    403,
    'managed-browser credentials cannot use the API',
  );
  const { authenticateToken: authenticate } = await import('../../src/modules/auth/service.ts');
  assert.equal((await authenticate(enrolled.token, { allowBrowser: true })).sessionId, quiet);
  assert.equal(
    (await control().read('owner')).credentials.some((c) => c.label === 'Managed browser'),
    false,
    'managed-browser credentials are not listed',
  );
  await control().update('owner', quiet, { state: 'stopped' });
  await assert.rejects(
    authenticate(enrolled.token, { allowBrowser: true }),
    { status: 401 },
    'the credential ends with its session',
  );
  console.log(
    'Control API passed: discovery privacy, role boundaries, credential/ticket revocation, takeover, all-path drain, queued creation and cancellation, stuck-command takeover, managed-browser credential scope.',
  );
} finally {
  server.closeAllConnections();
  await new Promise((resolve) => server.close(resolve));
  await control().store.close();
  await rm(directory, { recursive: true, force: true });
}
process.exit(0);
