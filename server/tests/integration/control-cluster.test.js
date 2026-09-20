/** Two actual HTTP/WebSocket server processes. Their RemoteBackend speaks the Postgres RPC contract, served here from SQLite. */
import assert from 'node:assert/strict';
import { fork } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import express from 'express';
import { WebSocket } from 'ws';
import { SqliteBackend } from '../../src/modules/control/store.ts';
const directories = [await mkdtemp(join(tmpdir(), 'oya-cluster-db-'))],
  children = [];
const backend = new SqliteBackend(join(directories[0], 'control.sqlite'), { lock: false });
const rpc = {
  control_load: (a) => backend.load(a.queries),
  control_commit: (a) => backend.commit(a),
  control_read_events: (a) =>
    backend.events({ project: a.target_project, after: a.after_seq, limit: a.lim, latest: a.latest, seqs: a.seqs }),
  control_prune: (a) => backend.prune(a.now_ms, a.cutoffs),
  control_begin: (a) => backend.beginCommand(a.session_id, a.actor, a.caller_instance),
  control_finish: (a) => backend.finishCommand(a.session_id, a.generation),
};
const app = express();
app.use(express.json({ limit: '5mb' }));
app.post('/', (req, res) => {
  try {
    res.json({ data: rpc[req.body.name](req.body.args || {}) ?? null });
  } catch (e) {
    res.json({ error: { message: e.code || e.message } });
  }
});
const server = await new Promise((resolve) => {
  const server = app.listen(0, '127.0.0.1', () => resolve(server));
});
const rpcUrl = `http://127.0.0.1:${server.address().port}/`;
async function start(name) {
  const directory = await mkdtemp(join(tmpdir(), 'oya-cluster-'));
  directories.push(directory);
  const child = fork(new URL('../support/cluster-node.js', import.meta.url), {
    env: {
      ...process.env,
      API_KEYS: 'cluster-owner',
      OYA_PROFILE_SECRET: 'cluster-test-secret',
      OYA_CLUSTER_SECRET: 'cluster-hop-secret',
      OYA_INSTANCE_ID: name,
      OYA_DATA_DIR: directory,
      OYA_TEST_RPC: rpcUrl,
      SUPABASE_URL: '',
      SUPABASE_SERVICE_KEY: '',
    },
    stdio: ['ignore', 'ignore', 'pipe', 'ipc'],
  });
  children.push(child);
  child.stderr.on('data', (bytes) => process.stderr.write(bytes));
  return new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('exit', (code) => reject(new Error(`Fixture exited ${code}`)));
    child.once('message', (value) => resolve({ child, url: value.url }));
  });
}
const call = async (url, path, method = 'GET', body, token = 'cluster-owner') => {
  const res = await fetch(url + '/api' + path, {
    method,
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    ...(body ? { body: JSON.stringify(body) } : {}),
  });
  return { status: res.status, body: await res.json() };
};
let ws;
try {
  const a = await start('replica-a'),
    b = await start('replica-b');
  const id = randomUUID();
  const seeded = new Promise((resolve) => a.child.once('message', resolve));
  a.child.send({ type: 'seed', id });
  await seeded;
  const result = await call(b.url, `/browsers/${id}/command`, 'POST', { action: 'analyze' });
  assert.equal(result.status, 200, JSON.stringify(result));
  assert.equal(result.body.data.owner, 'replica-a');
  // Only replica A holds the browser, so a successful MCP handshake through B proves the request was forwarded with its headers.
  const mcp = await fetch(`${b.url}/mcp/${id}`, {
    method: 'POST',
    headers: {
      Authorization: 'Bearer cluster-owner',
      'Content-Type': 'application/json',
      Accept: 'application/json, text/event-stream',
    },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method: 'initialize',
      params: { protocolVersion: '2025-03-26', capabilities: {}, clientInfo: { name: 'cluster-test', version: '1' } },
    }),
  });
  const handshake = await mcp.text();
  assert.equal(mcp.status, 200, handshake);
  assert.match(handshake, /serverInfo/);
  const credential = (await call(b.url, '/control/credentials', 'POST', { role: 'operator' })).body;
  ws = new WebSocket(b.url.replace('http:', 'ws:') + `/connect?browser=${id}`, {
    headers: { Authorization: `Bearer ${credential.token}` },
  });
  await new Promise((resolve, reject) => {
    ws.once('open', resolve);
    ws.once('error', reject);
  });
  const reply = new Promise((resolve) => ws.once('message', (raw) => resolve(JSON.parse(raw))));
  ws.send(JSON.stringify({ id: 1, method: 'Browser.getVersion' }));
  assert.equal((await reply).result.owner, 'replica-a');
  const closed = new Promise((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error('Revocation did not close the connection within five seconds')),
      5000,
    );
    ws.once('close', () => {
      clearTimeout(timeout);
      resolve();
    });
  });
  await call(b.url, `/control/credentials/${credential.id}`, 'DELETE');
  await closed;
  assert.equal((await call(a.url, '/control', 'GET', undefined, credential.token)).status, 401);
  console.log(
    'Cluster passed: cross-replica HTTP and MCP routing, raw CDP routing, preserved credential scope, revocation closes routed attachment within five seconds.',
  );
} finally {
  ws?.terminate();
  await Promise.all(
    children.map(
      (child) =>
        new Promise((resolve) => {
          if (child.exitCode !== null) return resolve();
          child.once('exit', resolve);
          child.kill('SIGTERM');
          setTimeout(() => child.kill('SIGKILL'), 5000).unref();
        }),
    ),
  );
  server.closeAllConnections();
  await new Promise((resolve) => server.close(resolve));
  backend.close();
  for (const directory of directories) await rm(directory, { recursive: true, force: true });
}
