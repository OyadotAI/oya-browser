#!/usr/bin/env node
/**
 * The pool MCP endpoint, end to end: an agent with nothing running starts a
 * browser, drives it and stops it, all through MCP. start_browser and
 * stop_browser replay the caller's own Authorization against the public API,
 * so this also runs the real admission, quota and persona path.
 *
 * Hermetic: a local Chrome, a local site, a server on a free port with its own
 * data dir, and dead database/sandbox settings.
 */

import { spawn } from 'child_process';
import { createServer } from 'http';
import { createServer as createNetServer } from 'net';
import { mkdtempSync, existsSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { fileURLToPath } from 'url';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js';
import { removeScratch } from '../support/scratch.js';

const CHROME = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
].find((p) => existsSync(p));
if (!CHROME) {
  console.log('⏭  No Chrome binary, skipping MCP lifecycle test');
  process.exit(0);
}

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
const freePort = () =>
  new Promise((resolve) => {
    const s = createNetServer().listen(0, '127.0.0.1', () => {
      const { port } = s.address();
      s.close(() => resolve(port));
    });
  });

const dir = mkdtempSync(join(tmpdir(), 'oya-mcp-'));
const chrome = spawn(
  CHROME,
  [
    '--headless=new',
    '--remote-debugging-port=0',
    '--no-first-run',
    `--user-data-dir=${join(dir, 'chrome')}`,
    'about:blank',
  ],
  { stdio: ['ignore', 'ignore', 'pipe'] },
);
const wsUrl = await new Promise((resolve, reject) => {
  let buf = '';
  const t = setTimeout(() => reject(new Error('Chrome did not report an endpoint')), 20000);
  chrome.stderr.on('data', (d) => {
    buf += d.toString();
    const m = buf.match(/ws:\/\/127\.0\.0\.1:\d+\/devtools\/browser\/[a-f0-9-]+/);
    if (m) {
      clearTimeout(t);
      resolve(m[0]);
    }
  });
});

const site = createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end('<!doctype html><title>MCP lifecycle</title><h1>Hello agent</h1><button>Press me</button>');
});
await new Promise((r) => site.listen(0, '127.0.0.1', r));
const siteUrl = `http://127.0.0.1:${site.address().port}/`;

const KEY = 'mcp-lifecycle-key';
const port = await freePort();
const base = `http://127.0.0.1:${port}`;
// Dead values rather than unset: dotenv fills only what is missing, so an
// unset variable would come back from server/.env with live credentials.
const server = spawn(process.execPath, ['src/index.ts'], {
  cwd: fileURLToPath(new URL('../..', import.meta.url)), // server/, where src/index.ts is
  env: {
    ...process.env,
    PORT: String(port),
    API_KEYS: KEY,
    OYA_DATA_DIR: join(dir, 'data'),
    OYA_PROFILE_SECRET: 'c'.repeat(64),
    OYA_ALLOW_PRIVATE_TARGETS: 'true',
    OYA_UI_MODE: '',
    SUPABASE_URL: '',
    SUPABASE_SERVICE_KEY: '',
    DAYTONA_API_KEY: '',
    OYA_API_KEY: '',
  },
  stdio: ['ignore', 'ignore', 'pipe'],
});
let serverLog = '';
server.stderr.on('data', (d) => {
  serverLog += d;
});

const api = (path, body) =>
  fetch(`${base}/api${path}`, {
    method: body ? 'POST' : 'GET',
    headers: { Authorization: `Bearer ${KEY}`, 'Content-Type': 'application/json' },
    body: body && JSON.stringify(body),
  }).then((r) => r.json());

try {
  for (let i = 0; i < 60; i++) {
    if (
      await fetch(`${base}/health`).then(
        (r) => r.ok,
        () => false,
      )
    )
      break;
    await new Promise((r) => setTimeout(r, 250));
  }
  await api('/config', { browser_provider: 'cdp', cdp_ws_url: wsUrl });

  const client = new Client({ name: 'test-mcp-lifecycle', version: '1.0.0' });
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

  console.log('\n2️⃣  The agent starts its own browser...');
  const started = await call('start_browser', { name: 'mcp-test', url: siteUrl });
  assert(!started.error && /ready/.test(started.text), `start_browser brings one up (${started.text.slice(0, 90)})`);
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
  const left = await api('/browsers');
  assert(Array.isArray(left) && left.length === 0, 'nothing is left running');
  const again = await call('analyze_page');
  assert(again.error && /start_browser/.test(again.text), 'and the tools say so');

  await client.close();
} catch (e) {
  failed++;
  console.log(`  ❌ threw: ${e.message}`);
  if (serverLog) console.log(serverLog.split('\n').slice(-15).join('\n'));
} finally {
  server.kill();
  chrome.kill('SIGKILL');
  site.close();
  await new Promise((r) => setTimeout(r, 300));
  removeScratch(dir);
}

console.log('\n' + '─'.repeat(50));
console.log(`  ${passed} passed, ${failed} failed`);
console.log('─'.repeat(50));
process.exit(failed ? 1 : 0);
