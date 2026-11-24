#!/usr/bin/env node
/** Remote MCP excludes an attached Oya desktop while REST retains native input and ownership gates. */

import { spawn } from 'child_process';
import { createServer } from 'http';
import { createServer as createNetServer } from 'net';
import { mkdtempSync } from 'fs';
import { once } from 'node:events';
import { WebSocket } from 'ws';
import { openNativeFixture } from '../support/native-browser.mjs';
import { tmpdir } from 'os';
import { join } from 'path';
import { fileURLToPath } from 'url';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js';
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
const freePort = () =>
  new Promise((resolve) => {
    const s = createNetServer().listen(0, '127.0.0.1', () => {
      const { port } = s.address();
      s.close(() => resolve(port));
    });
  });

const dir = mkdtempSync(join(tmpdir(), 'oya-mcp-'));
const site = createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end(`<!doctype html><title>MCP lifecycle</title><h1>Hello agent</h1>
<button onclick="document.title='Native click'; window.trusted=event.isTrusted; window.clicks=(window.clicks||0)+1">Press me</button>`);
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
    API_KEYS: KEY + ',mcp-other-key',
    OYA_DATA_DIR: join(dir, 'data'),
    OYA_PROFILE_SECRET: 'c'.repeat(64),
    OYA_ALLOW_PRIVATE_TARGETS: 'true',
    OYA_UI_MODE: '',
    SUPABASE_URL: '',
    SUPABASE_SERVICE_KEY: '',
    DAYTONA_API_KEY: '',
    OYA_API_KEY: '',
    OYA_CDP_WS_URL: '',
    OYA_BROWSER_PROVIDER: '',
  },
  stdio: ['ignore', 'ignore', 'pipe'],
});
const serverStopped = once(server, 'close');
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

let browser, socket, client;
try {
  browser = await openNativeFixture();
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

  // A desktop on this key must stay outside the remote pool.
  socket = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  const authenticated = new Promise((resolve, reject) => {
    const deadline = setTimeout(() => reject(Error('Native control authentication timed out')), 10000);
    socket.on('message', async (raw) => {
      const message = JSON.parse(raw);
      if (message.type === 'auth_ok') {
        clearTimeout(deadline);
        resolve(message);
      }
      if (message.type === 'ping') socket.send(JSON.stringify({ type: 'pong' }));
      if (message.type !== 'cmd') return;
      const result = await browser
        .send(message.action, message.params)
        .catch((error) => ({ ok: false, error: error.message }));
      socket.send(JSON.stringify({ ...result, type: 'cmd_result', id: message.id }));
    });
    socket.once('close', () => {
      clearTimeout(deadline);
      reject(Error('Native control socket closed'));
    });
    socket.once('error', (error) => {
      clearTimeout(deadline);
      reject(error);
    });
  });
  await once(socket, 'open');
  socket.send(
    JSON.stringify({
      type: 'auth',
      api_key: KEY,
      browser_id: 'native-mcp-fixture',
      browser_name: 'mcp-test',
      provider: 'oya-desktop',
      cdp: false,
      actions: ['navigate', 'analyze', 'click', 'handle_dialog'],
    }),
  );
  const identity = await authenticated;
  assert(identity.control.mode === 'agent', 'native socket is admitted under agent ownership');
  console.log('Checking that remote MCP leaves the connected desktop alone...');
  assert((await call('analyze_page')).error, 'remote pool cannot analyze the desktop');
  assert(!(await call('pool_status')).text.includes('mcp-test'), 'remote status hides the desktop');
  assert((await call('show_browser', { browser_id: 'native-mcp-fixture' })).error, 'remote card refuses desktop');
  assert((await call('stop_browser', { browser_id: 'native-mcp-fixture' })).error, 'remote stop refuses desktop');
  const started = await call('start_browser', { name: 'mcp-test', url: siteUrl });
  assert(started.error, 'unconfigured cloud start fails without borrowing the desktop');
  assert((await api('/browsers')).length === 1, 'desktop remains connected');
  const act = (action, params = {}) => api('/browsers/native-mcp-fixture/command', { action, params });
  const navigated = await act('navigate', { url: siteUrl });
  if (!navigated.ok) throw Error('Fixture navigation failed: ' + navigated.error);
  const page = await act('analyze');
  const button = page.data?.elements?.find((element) => element.text === 'Press me');
  if (!button?.id) throw Error(page.error || 'Native analysis did not find the fixture button');
  const click = () => act('click', { selector: String(button.id) });
  const clicked = await click();
  if (!clicked.ok) throw Error('Native REST click failed: ' + clicked.error);
  assert((await browser.evaluateMain('window.trusted')) === true, 'trusted native input still works through REST');
  /** Match a desktop ownership reply before attempting the next agent action. */
  const handoff = (action) =>
    new Promise((resolve, reject) => {
      const id = 'handoff-' + action;
      const timer = setTimeout(() => {
        socket.off('message', receive);
        reject(Error('Handoff timed out'));
      }, 10000);
      const receive = (raw) => {
        const result = JSON.parse(raw);
        if (result.type !== 'desktop_control_result' || result.id !== id) return;
        clearTimeout(timer);
        socket.off('message', receive);
        resolve(result);
      };
      socket.on('message', receive);
      socket.send(JSON.stringify({ type: 'desktop_control', id, action }));
    });
  assert((await handoff('acquire')).state?.mode === 'human', 'the human can take native browser ownership');
  const paused = await click();
  assert(paused.error, 'REST cannot act while the human owns the native browser');
  assert((await browser.evaluateMain('window.clicks')) === 1, 'the refused action never reaches the page');
  assert((await handoff('return')).state?.mode === 'agent', 'control returns to the agent');
  assert((await click()).ok, 'REST can act after explicit handback');
  assert((await browser.evaluateMain('window.clicks')) === 2, 'the resumed action reaches the page exactly once');
  const other = new Client({ name: 'other-owner', version: '1.0.0' });
  try {
    await other.connect(
      new StreamableHTTPClientTransport(new URL(`${base}/mcp/pool`), {
        requestInit: { headers: { Authorization: 'Bearer mcp-other-key' } },
      }),
    );
    const denied = await other.callTool({ name: 'analyze_page', arguments: {} });
    assert(denied.isError, 'another key cannot read the native page');
    const stopping = await other.callTool({ name: 'stop_browser', arguments: { browser_id: 'native-mcp-fixture' } });
    assert(stopping.isError, 'another key cannot stop the native browser');
    assert((await api('/browsers')).length === 1, 'unauthorized stop leaves the owner connected');
  } finally {
    await other.close();
  }
  console.log('\n3️⃣  ...and stops its control connection.');
  await api('/browsers/stop', { ids: ['native-mcp-fixture'] });
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
  await client?.close();
  socket?.terminate();
  await browser?.close();
  server.kill();
  const force = setTimeout(() => server.kill('SIGKILL'), 2000);
  await serverStopped;
  clearTimeout(force);
  await new Promise((resolve) => site.close(resolve));
  removeScratch(dir);
}

console.log('\n' + '─'.repeat(50));
console.log(`  ${passed} passed, ${failed} failed`);
console.log('─'.repeat(50));
process.exit(failed ? 1 : 0);
