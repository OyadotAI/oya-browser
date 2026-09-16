#!/usr/bin/env node
/**
 * Cloud browser provisioning + per-account config resolution.
 *
 * Covers:
 *  - POST /browsers/provision returns 409 (not 500) when Daytona is unconfigured
 *  - a sandbox is only deletable by the API key that created it, whether or not
 *    the browser is currently connected
 *  - keyConfig.resolve never pairs the deployment-wide OpenAI key with a
 *    tenant-supplied base URL (credential exfiltration)
 *
 * Usage:
 *   node test-sandbox.js
 */

import { createServer } from 'http';
import express from 'express';

import { mkdtempSync } from 'fs';
import { tmpdir } from 'os';
import { join as joinPath } from 'path';
// Never write through to the deployment's real data/ directory.
process.env.OYA_DATA_DIR = mkdtempSync(joinPath(tmpdir(), 'oya-test-'));
process.env.API_KEYS = 'admin-key,tenant-key';
// Force the no-database path so nothing here can write to Supabase or to
// data/config.json. runtimeConfig.set() persists for real -- never call it.
delete process.env.SUPABASE_URL;
delete process.env.SUPABASE_SERVICE_KEY;
// A distinctive server-wide default, so the fallback assertions below can tell
// "inherited the server value" apart from "returned an empty own value".
process.env.OPENAI_BASE_URL = 'https://server-wide.test/v1';
delete process.env.DAYTONA_API_KEY;
delete process.env.DAYTONA_SNAPSHOT;
delete process.env.OYA_PUBLIC_WS_URL;

const { router } = await import('./src/api.js');
const { isConfigured } = await import('./src/sandbox.js');
const { runtimeConfig } = await import('./src/runtime-config.js');
const keyConfig = await import('./src/key-config.js');

let passed = 0;
let failed = 0;
function assert(condition, label) {
  if (condition) { console.log(`  ✅ ${label}`); passed++; }
  else { console.log(`  ❌ ${label}`); failed++; }
}

const app = express();
app.use(express.json());
app.use('/', router);
const server = createServer(app);
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const base = `http://127.0.0.1:${server.address().port}`;

const call = async (path, method = 'GET', body, key = 'admin-key') => {
  const res = await fetch(base + path, {
    method,
    headers: { Authorization: `Bearer ${key}`, 'Content-Type': 'application/json' },
    ...(body ? { body: JSON.stringify(body) } : {}),
  });
  return { status: res.status, body: await res.json().catch(() => ({})) };
};

try {
  console.log('\n1️⃣  Provisioning is gated, not crashing...');
  assert(isConfigured({}) === false, 'isConfigured false with no Daytona settings');
  assert(
    isConfigured({ OYA_CLOUD_API_KEY: 'a', OYA_CLOUD_SNAPSHOT: 'b', OYA_PUBLIC_WS_URL: 'wss://x/ws' }) === true,
    'isConfigured true once all three are set',
  );
  assert(isConfigured({ OYA_CLOUD_API_KEY: 'a', OYA_CLOUD_SNAPSHOT: 'b' }) === false, 'partial config is not configured');
  // The runtime was renamed in public; deployments and CI still on the old
  // variable names must keep working rather than silently losing cloud browsers.
  assert(
    isConfigured({ DAYTONA_API_KEY: 'a', DAYTONA_SNAPSHOT: 'b', OYA_PUBLIC_WS_URL: 'wss://x/ws' }) === true,
    'the pre-rename variable names still configure Oya Cloud',
  );

  const unconfigured = await call('/browsers/provision', 'POST', { count: 2 });
  assert(unconfigured.status === 409, `unconfigured provision returns 409, not 500 (got ${unconfigured.status})`);
  assert(/OYA_CLOUD_API_KEY/.test(unconfigured.body.error || ''), 'error names the missing settings');
  assert(!/DAYTONA/.test(unconfigured.body.error || ''), 'and does so without naming the underlying vendor');

  const unauth = await fetch(base + '/browsers/provision', { method: 'POST' });
  assert(unauth.status === 401 || unauth.status === 403, `provision requires auth (got ${unauth.status})`);

  console.log('\n2️⃣  Sandbox deletion is owner-scoped...');
  // removeSandbox refuses before it ever reaches Daytona when no key is supplied.
  const { removeSandbox } = await import('./src/sandbox.js');
  let rejected = false;
  try { await removeSandbox('some-id', ''); } catch (e) { rejected = e.status === 400; }
  assert(rejected, 'removeSandbox requires an API key');

  const del = await call('/browsers/does-not-exist/sandbox', 'DELETE');
  assert(del.status === 409, `delete without Daytona configured returns 409 (got ${del.status})`);

  console.log('\n3.  The deployment-wide OpenAI key never reaches a tenant endpoint...');
  const serverKey = runtimeConfig.getOpenAIKey();
  const serverBase = runtimeConfig.getOpenAIBase();

  // The base URLs below are TEST-NET-3 literals (RFC 5737): validateBaseUrl
  // resolves whatever it is handed, so a made-up hostname would now be refused
  // for not resolving, and this suite has to pass with no network.
  //
  // A key that sets a base URL but has no LLM credential of its own must NOT
  // get the deployment-wide key pointed at its endpoint -- that would ship the
  // deployment's credential to an address the tenant controls.
  await keyConfig.set('k-no-key', { openai_base_url: 'https://203.0.113.10/v1' });
  const exfil = keyConfig.resolve('k-no-key');
  assert(exfil.baseUrl !== 'https://203.0.113.10/v1', 'a keyless tenant cannot redirect the deployment key');
  assert(exfil.baseUrl === serverBase, 'it falls back to the deployment-wide base URL');
  assert(exfil.openaiKey === serverKey, 'it still resolves the deployment-wide key');
  assert(!exfil.own, 'a key on the deployment-wide LLM credential stays under the chat token quota');

  // With its own credential, the key's own endpoint is honoured.
  await keyConfig.set('k-own', { openai_api_key: 'sk-own', openai_base_url: 'https://203.0.113.20/v1' });
  const own = keyConfig.resolve('k-own');
  assert(own.openaiKey === 'sk-own', "a key's own LLM credential is used");
  assert(own.baseUrl === 'https://203.0.113.20/v1', "a key's own base URL is honoured with its own credential");
  assert(own.own === true, 'a key with its own LLM credential is exempt from the chat token quota');

  await keyConfig.set('k-gemini', { llm_provider: 'gemini', openai_api_key: 'AIza-own' });
  const gemini = keyConfig.resolve('k-gemini');
  assert(gemini.baseUrl === 'https://generativelanguage.googleapis.com/v1beta/openai' && gemini.model === 'gemini-3.8-flash',
    'a Gemini key resolves to the Gemini endpoint and default model');

  await keyConfig.set('k-vertex', { llm_provider: 'vertex', openai_api_key: 'AIza-vertex' });
  const vertex = keyConfig.resolve('k-vertex');
  assert(vertex.baseUrl === 'https://aiplatform.googleapis.com/v1/publishers/google' && vertex.model === 'gemini-2.5-flash',
    'a Gemini Enterprise key resolves to the express-mode endpoint and default model');
  // A typo used to be stored and then silently resolve to the OpenAI defaults, which
  // looks like a broken key rather than a rejected setting.
  let refused = null;
  try { await keyConfig.set('k-vertex', { llm_provider: 'vertx' }); }
  catch (e) { refused = e; }
  assert(refused?.status === 400 && /openai, anthropic, gemini, vertex/.test(refused.message),
    'an unknown llm_provider is refused with the list of valid ones');
  assert(keyConfig.resolve('k-vertex').model === 'gemini-2.5-flash', 'the refused write left the previous provider in place');
  for (const [field, value] of [['browser_provider', 'not-a-provider'], ['captcha_solver', 'not-a-solver']]) {
    let bad = null;
    try { await keyConfig.set('k-vertex', { [field]: value }); } catch (e) { bad = e; }
    assert(bad?.status === 400, `an unknown ${field} is refused`);
  }
  // Every choice the UI and CLI offer must actually be accepted.
  for (const id of keyConfig.PROVIDER_CHOICES.map((p) => p.id)) await keyConfig.set('k-choices', { browser_provider: id });

  // The project-scoped enterprise endpoint is reached by overriding the base URL, which
  // is only honoured alongside the key's own credential.
  await keyConfig.set('k-vertex', { openai_base_url: 'https://us-central1-aiplatform.googleapis.com/v1/projects/p/locations/us-central1/endpoints/openapi' });
  assert(keyConfig.resolve('k-vertex').baseUrl.endsWith('/endpoints/openapi'),
    'a Gemini Enterprise key can point at a project-scoped endpoint instead');

  // A blank own base URL must be ignored by BOTH layers. Before the fix get()
  // used ?? and resolve() used ||, so the dashboard showed blank while requests
  // used the server-wide value.
  await keyConfig.set('k-blank', { openai_base_url: '' });
  assert(keyConfig.get('k-blank').openai_base_url === 'https://server-wide.test/v1',
    'a blank own base URL is ignored by get()');
  assert(keyConfig.resolve('k-blank').baseUrl === 'https://server-wide.test/v1',
    'a blank own base URL is ignored by resolve()');

  // Credentials are sealed at rest and never handed back in the clear.
  await keyConfig.set('k-secret', { openai_api_key: 'sk-supersecret-tail' });
  const shownSecret = keyConfig.get('k-secret').openai_api_key;
  assert(!shownSecret.includes('supersecret') && shownSecret.endsWith('tail'),
    'a stored LLM credential reads back masked');
  assert(keyConfig.resolve('k-secret').openaiKey === 'sk-supersecret-tail',
    'the server still resolves the plaintext');
  assert(keyConfig.envFor('k-secret').OPENAI_API_KEY === 'sk-supersecret-tail',
    'envFor layers the key\'s credential over the environment');

  // Re-saving the masked value the dashboard displays must not destroy the key.
  await keyConfig.set('k-secret', { openai_api_key: shownSecret });
  assert(keyConfig.resolve('k-secret').openaiKey === 'sk-supersecret-tail',
    'saving the masked placeholder leaves the real credential intact');

  // One key's settings are invisible to another.
  assert(keyConfig.resolve('k-other').openaiKey === serverKey,
    "another key sees none of this key's settings");

  console.log('\n4.  A tenant base URL cannot point the control plane inward (SSRF)...');
  const blocked = [
    'http://169.254.169.254/latest/meta-data',   // cloud metadata
    'https://127.0.0.1/v1', 'https://localhost/v1', 'https://[::1]/v1',
    'https://10.1.2.3/v1', 'https://192.168.1.1/v1', 'https://172.16.0.1/v1',
    'https://redis.internal/v1',
    'http://api.openai.com/v1',                   // plaintext
    'https://user:pass@api.openai.com/v1',        // embedded credentials
    'not-a-url',
  ];
  let blockedCount = 0;
  for (const bad of blocked) {
    try { await keyConfig.set('k-ssrf', { openai_base_url: bad }); }
    catch (e) { if (e.status === 400) blockedCount++; else console.log('    unexpected:', bad, e.message); }
  }
  assert(blockedCount === blocked.length, `all ${blocked.length} hostile base URLs rejected (got ${blockedCount})`);

  // An IP literal, not a hostname: validateBaseUrl resolves what it is given,
  // and this suite has to pass with no network.
  await keyConfig.set('k-ok', { openai_api_key: 'sk-x', openai_base_url: 'https://203.0.113.30/openai/v1' });
  const good = keyConfig.resolve('k-ok');
  assert(good.baseUrl === 'https://203.0.113.30/openai/v1', 'a legitimate https endpoint is still accepted');

  const { readFileSync } = await import('fs');
  const read = (f) => readFileSync(new URL(f, import.meta.url), 'utf8');
  assert(/redirect:\s*'error'/.test(read('./src/llm.js')), "the chat fetch refuses redirects (no 30x bypass)");
  // The guard is only worth anything while every LLM request goes through llm.js; a
  // caller that reaches for fetch again reintroduces the bypass.
  assert(['./src/chat-service.js', './src/playbook.js'].every((f) => !/fetch\(/.test(read(f))),
    'chat-service and playbook call the LLM through llm.js, not their own fetch');

  console.log('\nCloud sandbox lifecycle survives socket disconnects...');
  process.env.DAYTONA_API_KEY = 'isolated-daytona-key';
  process.env.DAYTONA_SNAPSHOT = 'test-snapshot';
  process.env.OYA_PUBLIC_WS_URL = 'wss://example.test/ws';
  const { Daytona } = await import('@daytona/sdk');
  const { createHash } = await import('node:crypto');
  const owner = createHash('sha256').update('tenant-key').digest('hex').slice(0, 32);
  let removed = false;
  const cloud = { id: 'sandbox-id', state: 'started', createdAt: new Date().toISOString(), labels: {
    'oya-browser': 'true', 'oya-browser-id': 'cloud-browser', 'oya-owner': owner,
  }, delete: async () => { removed = true; } };
  Daytona.prototype.list = async function* () { if (!removed) yield cloud; };
  Daytona.prototype.get = async function () { return cloud; };
  const disconnected = await call('/browsers', 'GET', undefined, 'tenant-key');
  assert(disconnected.body.some(b => b.id === 'cloud-browser' && b.health === 'dead'), 'running sandbox remains listed without a WebSocket');
  const foreign = await call('/browsers', 'GET', undefined, 'admin-key');
  assert(!foreign.body.some(b => b.id === 'cloud-browser'), 'sandbox inventory never crosses API-key ownership');
  const detail = await call('/browsers/cloud-browser', 'GET', undefined, 'tenant-key');
  assert(detail.status === 200 && detail.body.lastError?.includes('disconnected'), 'disconnected sandbox explains its state in the detail panel');
  const fleet = await call('/fleet', 'GET', undefined, 'tenant-key');
  assert(fleet.body.browsers.total === 1 && fleet.body.browsers.byHealth.dead === 1, 'fleet counters include disconnected sandboxes');
  const { listSandboxBrowsers } = await import('./src/sandbox.js');
  const reconnected = await listSandboxBrowsers('tenant-key', [{ id: 'cloud-browser', health: 'ok', name: 'Connected again' }]);
  assert(reconnected.length === 1 && reconnected[0].health === 'ok', 'reconnection replaces the disconnected row without duplication');
  const denied = await call('/browsers/cloud-browser/stop', 'POST', {}, 'admin-key');
  assert(!denied.body.ok && !removed, 'another owner cannot stop the disconnected sandbox');
  const stopped = await call('/browsers/cloud-browser/stop', 'POST', {}, 'tenant-key');
  assert(stopped.body.ok && removed, 'Stop deletes an owned sandbox even without a WebSocket');
  const afterStop = await call('/browsers', 'GET', undefined, 'tenant-key');
  assert(!afterStop.body.some(b => b.id === 'cloud-browser'), 'stopped sandbox disappears immediately from inventory');

  let launches = 0;
  let autoEntrypoint = true;
  Daytona.prototype.create = async function () { return { id: 'new-sandbox', setTtl: async () => {}, process: {
    executeCommand: async () => ({ result: 'ok' }),
    getEntrypointSession: async () => ({ commands: [{ command: autoEntrypoint ? "'/docker-entrypoint.sh'" : 'sleep infinity' }] }),
    createSession: async () => {}, executeSessionCommand: async () => { launches++; },
  } }; };
  const { createSandbox } = await import('./src/sandbox.js');
  await createSandbox({ apiKey: 'tenant-key' });
  assert(launches === 0, 'snapshot entrypoint is not launched twice');
  autoEntrypoint = false;
  await createSandbox({ apiKey: 'tenant-key' });
  assert(launches === 1, 'legacy sleeping snapshot still launches the browser');

  const { writeFileSync } = await import('node:fs');
  const { spawnSync } = await import('node:child_process');
  const preload = joinPath(process.env.OYA_DATA_DIR, 'delayed-database.mjs');
  writeFileSync(preload, `
    import { Server } from 'node:http';
    let keysLoaded = false;
    globalThis.fetch = async (url) => {
      const keys = String(url).includes('/api_keys');
      await new Promise(resolve => setTimeout(resolve, keys ? 150 : 10));
      if (keys) keysLoaded = true;
      // Each control RPC has its own return shape, and boot calls several. An
      // empty control_load is [[]] (one result array per query), and a commit
      // reports {ok}. Returning [] for everything made boot throw — which only
      // went unnoticed while an unhandled rejection during boot was swallowed.
      if (String(url).includes('/rpc/control_load')) return Response.json([[]]);
      if (String(url).includes('/rpc/control_commit')) return Response.json({ ok: true, events: [] });
      if (String(url).includes('/rpc/control_')) return Response.json([]);
      return Response.json(keys ? [{ key: 'registered-browser-key' }] : []);
    };
    Server.prototype.listen = function () { process.exit(keysLoaded ? 0 : 1); };
  `);
  const startup = spawnSync(process.execPath, ['--import', preload, 'src/index.js'], {
    env: { ...process.env, SUPABASE_URL: 'https://database.invalid', SUPABASE_SERVICE_KEY: 'isolated-db-key', OYA_UI_MODE: '', API_KEYS: '' },
    timeout: 10000, encoding: 'utf8',
  });
  assert(startup.status === 0, 'server waits for database API keys before accepting reconnects');

} finally {
  await new Promise((r) => server.close(r));
}

console.log('\n──────────────────────────────────────────────────');
console.log(`  ${passed} passed, ${failed} failed`);
console.log('──────────────────────────────────────────────────');
process.exit(failed ? 1 : 0);
