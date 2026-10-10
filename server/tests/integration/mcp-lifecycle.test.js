#!/usr/bin/env node
/**
 * The pool MCP endpoint, end to end: an agent with nothing running provisions a native cloud
 * browser, drives it and stops it, all through MCP. start_browser and
 * stop_browser replay the caller's own Authorization against the public API,
 * so this also runs the real admission, quota and persona path.
 *
 * Hermetic: a fake cloud allocator, real Oya native processes and enrollment,
 * a local site and a server with its own data directory. CDP provider parity
 * remains in cdp.test.js; this exercises Oya Cloud allocation rather than desktop adoption.
 */

import express from 'express';
import { WebSocketServer } from 'ws';
import { createServer } from 'http';
import { mkdtempSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js';
import { nativeProvider } from '../support/native-provider.mjs';
import { removeScratch } from '../support/scratch.js';

let passed = 0,
  failed = 0;
const assert = (ok, msg) => {
  if (ok) {
    passed++;
    console.log(`  ✅ ${msg}`);
  } else {
    failed++;
    console.log(`  ❌ ${msg}`);
  }
};
const dir = mkdtempSync(join(tmpdir(), 'oya-mcp-'));
const site = createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end('<!doctype html><title>MCP lifecycle</title><h1>Hello agent</h1><button>Press me</button>');
});
await new Promise((r) => site.listen(0, '127.0.0.1', r));
const siteUrl = `http://127.0.0.1:${site.address().port}/`;

const KEY = 'mcp-lifecycle-key';
Object.assign(process.env, {
  API_KEYS: KEY,
  OYA_DATA_DIR: join(dir, 'data'),
  OYA_PROFILE_SECRET: 'c'.repeat(64),
  OYA_ALLOW_PRIVATE_TARGETS: 'true',
  OYA_CLOUD_RUNTIME: 'docker',
  OYA_CLOUD_IMAGE: 'native-test-fixture',
});
const { router } = await import('../../src/app/api.ts');
const { handlePoolMcpRequest } = await import('../../src/mcp/server.ts');
const { handleConnection } = await import('../../src/modules/browsers/socket.ts');
const app = express();
app.use(express.json());
app.use('/api', router);
app.all('/mcp/pool', handlePoolMcpRequest);
const server = createServer(app);
const wss = new WebSocketServer({ server, path: '/ws' });
wss.on('connection', handleConnection);
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
process.env.OYA_PUBLIC_WS_URL = base.replace('http:', 'ws:') + '/ws';
const provider = await nativeProvider();

const api = (path, body) =>
  fetch(`${base}/api${path}`, {
    method: body ? 'POST' : 'GET',
    headers: { Authorization: `Bearer ${KEY}`, 'Content-Type': 'application/json' },
    body: body && JSON.stringify(body),
  }).then((r) => r.json());

let client;
try {
  await api('/config', { browser_provider: 'oya-cloud' });

  client = new Client({ name: 'test-mcp-lifecycle', version: '1.0.0' });
  await client.connect(
    new StreamableHTTPClientTransport(new URL(`${base}/mcp/pool`), {
      requestInit: { headers: { Authorization: `Bearer ${KEY}` } },
    }),
  );
  const call = async (name, args = {}) => {
    const r = await client.callTool({ name, arguments: args });
    return { error: !!r.isError, text: (r.content || []).map((c) => c.text || '').join('\n') };
  };

  console.log('\n1️⃣  Nothing running yet...');
  const tools = (await client.listTools()).tools.map((t) => t.name);
  assert(
    tools.includes('start_browser') && tools.includes('stop_browser'),
    'start_browser and stop_browser are listed',
  );
  const empty = await call('analyze_page');
  assert(empty.error && /start_browser/.test(empty.text), 'with no browser, the error says to call start_browser');
  assert(provider.counts.created === 0, 'no native worker exists before MCP start_browser');

  console.log('\n2️⃣  The agent starts its own browser...');
  const started = await call('start_browser', { name: 'mcp-test', url: siteUrl });
  assert(!started.error && /ready/.test(started.text), `start_browser brings one up (${started.text.slice(0, 90)})`);
  assert(provider.counts.created === 1, 'MCP start provisioned exactly one native provider session');
  const page = await call('analyze_page');
  assert(
    !page.error && /Hello agent/.test(page.text) && /Press me/.test(page.text),
    'it navigated, and analyze_page reads the page',
  );
  const status = await call('pool_status');
  assert(/★/.test(status.text) && /mcp-test/.test(status.text), 'pool_status shows it as the one being driven');

  console.log('\n3️⃣  ...and stops it.');
  const stopped = await call('stop_browser');
  assert(!stopped.error && /Stopped/.test(stopped.text), 'stop_browser stops it');
  assert(provider.counts.released === 1, 'MCP stop released the actual native provider session');
  const left = await api('/browsers');
  assert(Array.isArray(left) && left.length === 0, 'nothing is left running');
  const again = await call('analyze_page');
  assert(again.error && /start_browser/.test(again.text), 'and the tools say so');

  await client.close();
} catch (e) {
  failed++;
  console.log(`  ❌ threw: ${e.message}`);
} finally {
  await client?.close();
  await provider.close();
  wss.close();
  await new Promise((resolve) => server.close(resolve));
  await new Promise((resolve) => site.close(resolve));
  removeScratch(dir);
}

console.log('\n' + '─'.repeat(50));
console.log(`  ${passed} passed, ${failed} failed`);
console.log('─'.repeat(50));
process.exit(failed ? 1 : 0);
