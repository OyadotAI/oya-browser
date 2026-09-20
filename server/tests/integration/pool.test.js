#!/usr/bin/env node
/**
 * Integration test for pool, round-robin, and cookie sync.
 *
 * Usage:
 *   node test-pool.js
 *
 * Starts the server on a random port, connects 3 fake browsers,
 * and verifies cookie sync + round-robin dispatch.
 */

import { createServer } from 'http';
import express from 'express';
import { WebSocketServer, WebSocket } from 'ws';
import { randomUUID } from 'crypto';

// ── Inline server setup (same as index.js but on a random port) ──────────

process.env.FLEET_TOKEN = 'test-fleet-token';
import { mkdtempSync } from 'fs';
import { tmpdir } from 'os';
import { join as joinPath } from 'path';
// Never write through to the deployment's real data/ directory.
process.env.OYA_DATA_DIR = mkdtempSync(joinPath(tmpdir(), 'oya-test-'));
process.env.API_KEYS = 'admin-key-for-testing';
process.env.OYA_OPERATOR_TOKEN = 'operator-token-for-testing';

const { router: apiRouter } = await import('../../src/app/api.ts');
const { handleConnection } = await import('../../src/modules/browsers/socket.ts');
const { handleMcpRequest, handlePoolMcpRequest } = await import('../../src/mcp/server.ts');
const { registry } = await import('../../src/modules/browsers/registry.ts');

const app = express();
app.use(express.json());
app.use(apiRouter);
app.post('/mcp/pool', handlePoolMcpRequest);
app.get('/mcp/pool', handlePoolMcpRequest);
app.post('/mcp/:browserId', handleMcpRequest);

const server = createServer(app);
server.timeout = 0;
const wss = new WebSocketServer({ server, path: '/ws', perMessageDeflate: false });
wss.on('connection', (ws) => handleConnection(ws));

const PORT = await new Promise((resolve) => {
  server.listen(0, () => resolve(server.address().port));
});
console.log(`\n🧪 Test server on port ${PORT}\n`);

// ── Helpers ──────────────────────────────────────────────────────────────

let passed = 0;
let failed = 0;

function assert(condition, label) {
  if (condition) {
    console.log(`  ✅ ${label}`);
    passed++;
  } else {
    console.log(`  ❌ ${label}`);
    failed++;
  }
}

function wait(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

/** Connect a fake browser via WebSocket, returns { ws, browserId, messages[] } */
function connectBrowser(name) {
  return new Promise((resolve, reject) => {
    const browserId = randomUUID();
    const messages = [];
    const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws`);

    ws.on('open', () => {
      ws.send(
        JSON.stringify({
          type: 'auth',
          api_key: 'test-fleet-token',
          browser_id: browserId,
          browser_name: name,
        }),
      );
    });

    ws.on('message', (raw) => {
      const msg = JSON.parse(raw.toString());
      messages.push(msg);

      if (msg.type === 'auth_ok') {
        resolve({ ws, browserId, name, messages });
      }

      // Auto-respond to commands with a simple ack (simulates extension)
      if (msg.type === 'cmd') {
        const response = { type: 'cmd_result', id: msg.id, ok: true, data: {} };

        if (msg.action === 'analyze') {
          response.data = {
            markdown: `# Page on ${name}`,
            elements: [
              { id: 1, type: 'button', text: 'Submit', visible: true },
              { id: 2, type: 'input', text: '', visible: true },
            ],
            scroll: { top: 0 },
            viewport: { width: 1280, height: 720 },
          };
        } else if (msg.action === 'navigate') {
          response.data = { url: msg.params.url, title: `Page - ${name}` };
        } else if (msg.action === 'click') {
          response.data = { clicked: msg.params.selector };
        }

        ws.send(JSON.stringify(response));
      }

      // Auto-respond to pings
      if (msg.type === 'ping') {
        ws.send(JSON.stringify({ type: 'pong' }));
      }
    });

    ws.on('error', reject);
    setTimeout(() => reject(new Error(`${name} auth timeout`)), 5000);
  });
}

async function httpPost(path, body, key = 'test-fleet-token') {
  const headers = { 'Content-Type': 'application/json', Authorization: `Bearer ${key}` };
  // MCP Streamable HTTP requires Accept header
  if (path.startsWith('/mcp/')) {
    headers['Accept'] = 'application/json, text/event-stream';
  }
  const res = await fetch(`http://127.0.0.1:${PORT}${path}`, {
    method: 'POST',
    headers,
    body: JSON.stringify(body),
  });
  const contentType = res.headers.get('content-type') || '';
  if (contentType.includes('text/event-stream')) {
    // Parse SSE: extract JSON from "data: ..." lines
    const text = await res.text();
    const lines = text.split('\n').filter((l) => l.startsWith('data: '));
    const events = lines.map((l) => JSON.parse(l.slice(6)));
    // Return the last event (usually the response)
    return { status: res.status, data: events[events.length - 1] || {} };
  }
  return { status: res.status, data: await res.json() };
}

async function httpGet(path, key = 'test-fleet-token') {
  const res = await fetch(`http://127.0.0.1:${PORT}${path}`, {
    headers: { Authorization: `Bearer ${key}` },
  });
  return { status: res.status, data: await res.json() };
}

// ── Tests ────────────────────────────────────────────────────────────────

try {
  // 1. Connect 3 fake browsers
  console.log('1️⃣  Connecting browsers...');
  const b1 = await connectBrowser('Browser-1');
  const b2 = await connectBrowser('Browser-2');
  const b3 = await connectBrowser('Browser-3');
  await wait(500); // let cookie_sync messages settle

  assert(registry.isConnected(b1.browserId), 'Browser-1 connected');
  assert(registry.isConnected(b2.browserId), 'Browser-2 connected');
  assert(registry.isConnected(b3.browserId), 'Browser-3 connected');

  // 2. Pool status
  console.log('\n2️⃣  Pool status...');
  const pool = await httpGet('/pool');
  assert(pool.status === 200, 'GET /pool returns 200');
  assert(pool.data.size === 3, `Pool has 3 browsers (got ${pool.data.size})`);

  // 3. Round-robin via REST
  console.log('\n3️⃣  Round-robin commands...');
  const rr1 = await httpPost('/pool/command', { action: 'navigate', params: { url: 'https://example.com' } });
  const rr2 = await httpPost('/pool/command', { action: 'navigate', params: { url: 'https://example.com' } });
  const rr3 = await httpPost('/pool/command', { action: 'navigate', params: { url: 'https://example.com' } });

  assert(rr1.data.ok === true, 'Round-robin cmd 1 OK');
  assert(rr2.data.ok === true, 'Round-robin cmd 2 OK');
  assert(rr3.data.ok === true, 'Round-robin cmd 3 OK');

  const browsers = [rr1.data._browser, rr2.data._browser, rr3.data._browser];
  const unique = new Set(browsers);
  assert(
    unique.size === 3,
    `Commands hit 3 different browsers (got ${unique.size}: ${browsers.map((b) => b.slice(0, 8)).join(', ')})`,
  );

  // 4th command should wrap around
  const rr4 = await httpPost('/pool/command', { action: 'navigate', params: { url: 'https://example.com' } });
  assert(rr4.data._browser === browsers[0], 'Round-robin wraps around to first browser');

  // 4. Cookie sync
  console.log('\n4️⃣  Cookie sync...');

  // Browser-1 sends a cookie change
  b1.ws.send(
    JSON.stringify({
      type: 'cookie_changed',
      change: {
        removed: false,
        cookie: {
          name: 'session',
          value: 'abc123',
          domain: '.example.com',
          path: '/',
          secure: true,
          httpOnly: true,
          sameSite: 'lax',
        },
      },
    }),
  );

  await wait(500);

  // Sync is pull-based: nothing is pushed to peers, because fanning every
  // change out to every browser is quadratic in pool size.
  assert(
    b2.messages.every((m) => m.type !== 'cookie_update'),
    'Browser-2 is NOT pushed the change',
  );
  assert(
    b3.messages.every((m) => m.type !== 'cookie_update'),
    'Browser-3 is NOT pushed the change',
  );

  // Browser-2 pulls it when it is about to visit that host.
  b2.messages.length = 0;
  b2.ws.send(JSON.stringify({ type: 'cookie_pull', domains: ['www.example.com'], pullId: 'test-1' }));
  await wait(300);
  const pulled = b2.messages.find((m) => m.type === 'cookie_sync' && m.pullId === 'test-1');
  assert(pulled != null, 'Browser-2 receives cookie_sync in response to its pull');
  assert(pulled?.cookies?.find((c) => c.name === 'session')?.value === 'abc123', 'Pulled cookie has the right value');

  // A host with nothing stored gets nothing back.
  b2.messages.length = 0;
  b2.ws.send(JSON.stringify({ type: 'cookie_pull', domains: ['unrelated.test'], pullId: 'test-2' }));
  await wait(300);
  const empty = b2.messages.find((m) => m.type === 'cookie_sync' && m.pullId === 'test-2');
  assert(empty != null && empty.cookies.length === 0, 'A pull for an unrelated host returns nothing');

  // Batched changes are accepted too.
  b1.ws.send(
    JSON.stringify({
      type: 'cookie_changed',
      changes: [
        {
          removed: false,
          cookie: {
            name: 'batched',
            value: 'v1',
            domain: '.example.com',
            path: '/',
            secure: true,
            httpOnly: false,
            sameSite: 'lax',
          },
        },
      ],
    }),
  );
  await wait(300);
  b2.messages.length = 0;
  b2.ws.send(JSON.stringify({ type: 'cookie_pull', domains: ['example.com'], pullId: 'test-3' }));
  await wait(300);
  const batched = b2.messages.find((m) => m.type === 'cookie_sync' && m.pullId === 'test-3');
  assert(
    batched?.cookies?.some((c) => c.name === 'batched'),
    'A batched cookie_changed reaches the jar',
  );

  // Check server cookie jar via REST
  const jar = await httpGet('/pool/cookies');
  assert(jar.data.cookies.length >= 1, `Cookie jar has cookies (${jar.data.cookies.length})`);
  const sessionCookie = jar.data.cookies.find((c) => c.name === 'session');
  assert(sessionCookie?.value === 'abc123', 'Server jar has the synced cookie');

  // 5. Cookie dump on connect
  console.log('\n5️⃣  Cookie dump on new browser connect...');

  // Connect a 4th browser that sends a cookie dump
  const b4 = await connectBrowser('Browser-4');
  b4.ws.send(
    JSON.stringify({
      type: 'cookie_dump',
      cookies: [
        {
          name: 'token',
          value: 'xyz789',
          domain: '.test.com',
          path: '/',
          secure: false,
          httpOnly: false,
          sameSite: 'none',
        },
      ],
    }),
  );
  await wait(500);

  b1.messages.length = 0; // only look at what arrives after the dump
  // A dump merges into the jar and is NOT fanned out — pushing a full jar to
  // every browser on every connect was the worst of the quadratic paths.
  const b1SyncMsg = b1.messages.find((m) => m.type === 'cookie_sync' && m.cookies?.some((c) => c.name === 'token'));
  assert(b1SyncMsg == null, 'Browser-4 dump is NOT broadcast to Browser-1');

  // Browser-1 gets it by pulling the host, like any other change.
  b1.messages.length = 0;
  b1.ws.send(JSON.stringify({ type: 'cookie_pull', domains: ['test.com'], pullId: 'dump-1' }));
  await wait(300);
  const afterDump = b1.messages.find((m) => m.type === 'cookie_sync' && m.pullId === 'dump-1');
  assert(
    afterDump?.cookies?.some((c) => c.name === 'token'),
    "Browser-1 pulls Browser-4's dumped cookie on demand",
  );

  const jar2 = await httpGet('/pool/cookies');
  const hasToken = jar2.data.cookies.some((c) => c.name === 'token' && c.value === 'xyz789');
  assert(hasToken, 'Server jar has cookie from Browser-4 dump');

  // 6. Pool MCP endpoint
  console.log('\n6️⃣  Pool MCP endpoint...');

  // Call pool_status tool via MCP
  const mcpRes = await httpPost('/mcp/pool', {
    jsonrpc: '2.0',
    id: 1,
    method: 'tools/list',
    params: {},
  });
  assert(mcpRes.status === 200, 'MCP pool tools/list returns 200');
  const tools = mcpRes.data?.result?.tools || [];
  const toolNames = tools.map((t) => t.name);
  assert(toolNames.includes('analyze_page'), 'Pool has analyze_page tool');
  assert(toolNames.includes('navigate'), 'Pool has navigate tool');
  assert(toolNames.includes('click'), 'Pool has click tool');
  assert(toolNames.includes('pool_status'), 'Pool has pool_status tool');

  // Call analyze_page via MCP pool
  const analyzeRes = await httpPost('/mcp/pool', {
    jsonrpc: '2.0',
    id: 2,
    method: 'tools/call',
    params: { name: 'analyze_page', arguments: {} },
  });
  assert(analyzeRes.status === 200, 'MCP pool analyze_page returns 200');
  const analyzeText = analyzeRes.data?.result?.content?.[0]?.text || '';
  assert(analyzeText.includes('# Page on'), `analyze_page returned page content: "${analyzeText.slice(0, 60)}..."`);
  assert(/^\[.+\]\n/.test(analyzeText), 'Pool replies name the browser that answered');

  // 7. Fleet provision
  console.log('\n7️⃣  Fleet provision...');
  // Minting credentials is a host operation now, not something an API key can do.
  const denied = await httpPost('/fleet/provision?count=5', {}, 'admin-key-for-testing');
  assert(denied.status === 403, `An API key cannot mint keys (got ${denied.status})`);
  const prov = await httpPost('/fleet/provision?count=5', {}, 'operator-token-for-testing');
  assert(prov.status === 200, 'Provision returns 200');
  assert(prov.data.keys?.length === 5, `Provisioned 5 keys (got ${prov.data.keys?.length})`);
  assert(prov.data.keys[0].length > 20, 'Keys are sufficiently long');

  // 8. Disconnect and verify pool shrinks
  console.log('\n8️⃣  Disconnect handling...');
  b4.ws.close();
  await wait(500);
  const pool2 = await httpGet('/pool');
  assert(pool2.data.size === 3, `Pool shrunk to 3 after disconnect (got ${pool2.data.size})`);

  // ── Summary ──

  console.log(`\n${'─'.repeat(50)}`);
  console.log(`  ${passed} passed, ${failed} failed`);
  console.log(`${'─'.repeat(50)}\n`);

  // Cleanup
  b1.ws.close();
  b2.ws.close();
  b3.ws.close();
  await wait(200);
  server.close();
  process.exit(failed > 0 ? 1 : 0);
} catch (err) {
  console.error('\n💥 Test crashed:', err);
  server.close();
  process.exit(1);
}
