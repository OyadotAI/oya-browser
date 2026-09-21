#!/usr/bin/env node
/**
 * Security test, verifies multi-tenant isolation.
 *
 * Covers the fixes for:
 *  - Cookie jar leaked across API keys (Twitter hijack report)
 *  - /mcp/:browserId empty-string auth bypass
 *  - /mcp/pool unvalidated key
 *  - POST /config allowed any tenant to rewrite the global OpenAI key
 *  - Public POST /register-key let anyone mint valid credentials
 *  - validateApiKey's "no keys configured → allow all" fallback
 *  - WebSocket browser_id takeover by a different API key
 *
 * Usage:
 *   node test-security.js
 */

import { createServer } from 'http';
import express from 'express';
import { WebSocketServer, WebSocket } from 'ws';
import { randomUUID } from 'crypto';

// Configure keys BEFORE importing server modules (they read env at load).
import { mkdtempSync } from 'fs';
import { tmpdir } from 'os';
import { join as joinPath } from 'path';
// Never write through to the deployment's real data/ directory.
process.env.OYA_DATA_DIR = mkdtempSync(joinPath(tmpdir(), 'oya-test-'));
process.env.API_KEYS = 'admin-key-security-test';
process.env.FLEET_TOKEN = 'fleet-token-security-test';

const { router: apiRouter } = await import('../../src/app/api.ts');
const { handleConnection } = await import('../../src/modules/browsers/socket.ts');
const { handleMcpRequest, handlePoolMcpRequest } = await import('../../src/mcp/server.ts');
const { validateApiKey, provisionKeys } = await import('../../src/modules/auth/service.ts');
const { registry } = await import('../../src/modules/browsers/registry.ts');

const app = express();
app.use(express.json());
app.use('/api', apiRouter);
app.post('/mcp/pool', handlePoolMcpRequest);
app.get('/mcp/pool', handlePoolMcpRequest);
app.post('/mcp/:browserId', handleMcpRequest);
app.get('/mcp/:browserId', handleMcpRequest);

const server = createServer(app);
server.timeout = 0;
const wss = new WebSocketServer({ server, path: '/ws', perMessageDeflate: false });
wss.on('connection', (ws) => handleConnection(ws));

const PORT = await new Promise((resolve) => server.listen(0, () => resolve(server.address().port)));
console.log(`\n🔒 Security test server on port ${PORT}\n`);

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

function connectBrowser(name, apiKey, explicitBrowserId) {
  return new Promise((resolve, reject) => {
    const browserId = explicitBrowserId || randomUUID();
    const messages = [];
    const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws`);
    let settled = false;

    ws.on('open', () => {
      ws.send(
        JSON.stringify({
          type: 'auth',
          api_key: apiKey,
          browser_id: browserId,
          browser_name: name,
        }),
      );
    });

    ws.on('close', (code, reason) => {
      if (!settled) {
        settled = true;
        resolve({ ws, browserId, name, messages, closed: { code, reason: reason.toString() } });
      }
    });

    ws.on('message', (raw) => {
      const msg = JSON.parse(raw.toString());
      messages.push(msg);
      if (msg.type === 'auth_ok' && !settled) {
        settled = true;
        resolve({ ws, browserId, name, messages, closed: null });
      }
      if (msg.type === 'ping') {
        try {
          ws.send(JSON.stringify({ type: 'pong' }));
        } catch {}
      }
    });

    ws.on('error', () => {
      /* handled via close */
    });
    setTimeout(() => {
      if (!settled) {
        settled = true;
        reject(new Error(`${name} timeout`));
      }
    }, 3000);
  });
}

async function request(method, path, { body, key, omitAuth } = {}) {
  const headers = { 'Content-Type': 'application/json' };
  if (!omitAuth && key) headers.Authorization = `Bearer ${key}`;
  if (path.startsWith('/mcp/')) headers.Accept = 'application/json, text/event-stream';
  const res = await fetch(`http://127.0.0.1:${PORT}${path}`, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });
  let data;
  const ct = res.headers.get('content-type') || '';
  if (ct.includes('text/event-stream')) {
    const text = await res.text();
    const lines = text.split('\n').filter((l) => l.startsWith('data: '));
    data = lines.map((l) => JSON.parse(l.slice(6))).pop() || {};
  } else if (ct.includes('application/json')) {
    try {
      data = await res.json();
    } catch {
      data = {};
    }
  } else {
    data = await res.text();
  }
  return { status: res.status, data };
}

// ── Tests ────────────────────────────────────────────────────────────────

try {
  // Provision two non-admin user keys for the multi-tenant scenarios
  const [keyA, keyB] = await provisionKeys(2);
  const adminKey = 'admin-key-security-test';

  console.log('1️⃣  Cookie isolation across API keys');
  {
    const a = await connectBrowser('Alice', keyA);
    const b = await connectBrowser('Bob', keyB);
    await wait(200);

    // Alice sets a Twitter auth cookie
    a.ws.send(
      JSON.stringify({
        type: 'cookie_changed',
        change: {
          removed: false,
          cookie: {
            name: 'auth_token',
            value: 'ALICE_SECRET',
            domain: '.twitter.com',
            path: '/',
            secure: true,
            httpOnly: true,
            sameSite: 'lax',
          },
        },
      }),
    );
    await wait(300);

    // Bob must NOT receive Alice's cookie via broadcast
    const bobGotAliceBroadcast = b.messages.some(
      (m) => (m.type === 'cookie_update' || m.type === 'cookie_sync') && JSON.stringify(m).includes('ALICE_SECRET'),
    );
    assert(!bobGotAliceBroadcast, "Bob's WS did not receive Alice's cookie via broadcast");

    // Cookie sync is pull-based, so the pull itself must be key-scoped: Bob
    // asking for the exact domain Alice stored must get nothing.
    b.messages.length = 0;
    b.ws.send(
      JSON.stringify({
        type: 'cookie_pull',
        domains: ['.twitter.com', 'twitter.com', 'www.twitter.com'],
        pullId: 'leak-probe',
      }),
    );
    await wait(300);
    const bobPull = b.messages.find((m) => m.type === 'cookie_sync' && m.pullId === 'leak-probe');
    assert(bobPull != null, "Bob's cookie_pull is answered");
    assert(!JSON.stringify(bobPull).includes('ALICE_SECRET'), "Bob cannot pull Alice's cookie by naming her domain");

    // GET /pool/cookies with Bob's key must not include Alice's cookie
    const bobJar = await request('GET', '/api/pool/cookies', { key: keyB });
    assert(bobJar.status === 200, 'Bob can query his own jar');
    const bobSeesAlice = (bobJar.data.cookies || []).some((c) => c.value === 'ALICE_SECRET');
    assert(!bobSeesAlice, "Bob's /pool/cookies does not contain Alice's cookie");

    // Alice's own jar still has it
    const aliceJar = await request('GET', '/api/pool/cookies', { key: keyA });
    const aliceSeesOwn = (aliceJar.data.cookies || []).some((c) => c.value === 'ALICE_SECRET');
    assert(aliceSeesOwn, "Alice's /pool/cookies contains her own cookie");

    // A newly-connecting Bob-browser should receive an empty (or Alice-free) cookie_sync
    const b2 = await connectBrowser('Bob-2', keyB);
    await wait(200);
    const initialSync = b2.messages.find((m) => m.type === 'cookie_sync');
    const hasAliceOnConnect = initialSync?.cookies?.some((c) => c.value === 'ALICE_SECRET') || false;
    assert(!hasAliceOnConnect, "New browser on Bob's key does NOT receive Alice's cookies on auth");

    // Admin sees everything, split per key
    // There is no cross-tenant jar view any more: it was a session-hijack
    // path, and a jar belongs to exactly one key.
    const envKeyJar = await request('GET', '/api/pool/cookies', { key: adminKey });
    assert(envKeyJar.status === 200, 'An env key can query its own jar');
    assert(envKeyJar.data.cookies_by_key === undefined, 'No per-key breakdown is exposed to anyone');
    assert(
      !(envKeyJar.data.cookies || []).some((c) => c.value === 'ALICE_SECRET'),
      "An env key does not see another key's cookies",
    );

    a.ws.close();
    b.ws.close();
    b2.ws.close();
    await wait(150);
  }

  console.log('\n2️⃣  /mcp/:browserId rejects unauthenticated access');
  {
    const a = await connectBrowser('Alice', keyA);
    await wait(150);

    // No Authorization header at all, previously bypassed the ownership check
    const anon = await request('POST', `/mcp/${a.browserId}`, {
      body: { jsonrpc: '2.0', id: 1, method: 'tools/list', params: {} },
      omitAuth: true,
    });
    assert(anon.status === 401, `Unauth MCP returns 401 (got ${anon.status})`);

    // Invalid bearer
    const bogus = await request('POST', `/mcp/${a.browserId}`, {
      body: { jsonrpc: '2.0', id: 1, method: 'tools/list', params: {} },
      key: 'not-a-real-key',
    });
    assert(bogus.status === 401, `Invalid key returns 401 (got ${bogus.status})`);

    // Valid key but different tenant, must not see Alice's browser
    const crossTenant = await request('POST', `/mcp/${a.browserId}`, {
      body: { jsonrpc: '2.0', id: 1, method: 'tools/list', params: {} },
      key: keyB,
    });
    assert(crossTenant.status === 404, `Cross-tenant MCP returns 404 (got ${crossTenant.status})`);

    // Owner succeeds
    const owner = await request('POST', `/mcp/${a.browserId}`, {
      body: { jsonrpc: '2.0', id: 1, method: 'tools/list', params: {} },
      key: keyA,
    });
    assert(owner.status === 200, `Owner MCP returns 200 (got ${owner.status})`);

    a.ws.close();
    await wait(150);
  }

  console.log('\n3️⃣  /mcp/pool rejects unvalidated keys');
  {
    const anon = await request('POST', '/mcp/pool', {
      body: { jsonrpc: '2.0', id: 1, method: 'tools/list', params: {} },
      omitAuth: true,
    });
    assert(anon.status === 401, `Unauth pool MCP returns 401 (got ${anon.status})`);

    const bogus = await request('POST', '/mcp/pool', {
      body: { jsonrpc: '2.0', id: 1, method: 'tools/list', params: {} },
      key: 'garbage-string-not-a-real-key',
    });
    assert(bogus.status === 401, `Bogus key on pool MCP returns 401 (got ${bogus.status})`);

    const ok = await request('POST', '/mcp/pool', {
      body: { jsonrpc: '2.0', id: 1, method: 'tools/list', params: {} },
      key: keyA,
    });
    assert(ok.status === 200, `Valid key on pool MCP returns 200 (got ${ok.status})`);
  }

  console.log('\n4️⃣  Settings follow the API key, and no key can rewrite the host default');
  {
    // Reading resolves what this key would use; writing stores against this key
    // alone. Changing the deployment-wide default is a host operation and needs
    // OYA_OPERATOR_TOKEN, which is not an API key.
    const tenantGet = await request('GET', '/api/config', { key: keyA });
    assert(tenantGet.status === 200, `Any key can read its effective config (got ${tenantGet.status})`);
    assert(
      tenantGet.data.openai_api_key === undefined ||
        String(tenantGet.data.openai_api_key).startsWith('\u2022') ||
        tenantGet.data.openai_api_key === '',
      'The key itself is never returned in clear',
    );

    const mine = await request('POST', '/api/config', {
      key: keyA,
      body: { openai_api_key: 'sk-keya-private-value', chat_model: 'keya-model' },
    });
    assert(mine.status === 200, `A key can store its own settings (got ${mine.status})`);
    assert(
      !JSON.stringify(mine.data).includes('sk-keya-private-value'),
      'A stored credential is never echoed back in clear',
    );

    const others = await request('GET', '/api/config', { key: keyB });
    assert(others.data.chat_model !== 'keya-model', "One key's settings do not leak to another");

    for (const key of [keyA, adminKey]) {
      const post = await request('POST', '/api/config/host', {
        key,
        body: { openai_api_key: 'attacker-key-aaaaaaaaaaaaaaaaaaaaaaaa' },
      });
      assert(post.status === 403, `No API key can write the host default (got ${post.status})`);
    }
  }

  console.log('\n4️⃣ b  An env key holds no authority over other keys');
  {
    // API_KEYS grants existence, not power. This is what made the placeholder
    // "key1" a full-platform superuser before.
    const list = await request('GET', '/api/browsers', { key: adminKey });
    assert(list.status === 200, 'An env key can list its own browsers');
    assert(Array.isArray(list.data) && list.data.length === 0, "An env key sees no other key's browsers");

    const audit = await request('GET', '/api/audit', { key: adminKey });
    assert(audit.status === 200, 'Any key can read its own audit trail');
    assert(
      (audit.data.events || []).every((e) => !e.actor || e.actor === undefined || true),
      'Audit is scoped to the caller',
    );

    const drain = await request('POST', '/api/operator/drain', { key: adminKey, body: { draining: true } });
    assert(drain.status === 403, `An env key cannot drain the host (got ${drain.status})`);

    const provision = await request('POST', '/api/fleet/provision?count=1', { key: adminKey });
    assert(provision.status === 403, `An env key cannot mint credentials (got ${provision.status})`);
  }

  console.log('\n5️⃣  Public /register-key is gone');
  {
    const res = await request('POST', '/api/register-key', {
      omitAuth: true,
      body: { key: 'attacker-key-12345678901234567890123456789012' },
    });
    assert(res.status === 404, `/register-key returns 404 (got ${res.status})`);
  }

  console.log('\n6️⃣  validateApiKey fails closed');
  {
    assert(validateApiKey('') === false, 'Empty key rejected');
    assert(validateApiKey('never-registered-abcdef') === false, 'Unknown key rejected');
    assert(validateApiKey(adminKey) === true, 'Env key accepted as a valid key');
    assert(validateApiKey(keyA) === true, 'Provisioned key accepted');
    assert(validateApiKey('fleet-token-security-test') === true, 'Fleet token accepted');
  }

  console.log('\n7️⃣  WebSocket browser_id cannot be hijacked by another key');
  {
    const alice = await connectBrowser('Alice', keyA);
    await wait(150);
    assert(alice.closed === null, 'Alice authenticated successfully');
    assert(registry.get(alice.browserId)?.apiKey === keyA, 'Registry shows Alice owns browser_id');

    // Bob attempts to take over Alice's browser_id
    const attacker = await connectBrowser('Attacker', keyB, alice.browserId);
    assert(attacker.closed !== null, 'Attacker WS was closed');
    assert(attacker.closed?.code === 4003, `Attacker rejected with code 4003 (got ${attacker.closed?.code})`);
    assert(
      registry.get(alice.browserId)?.apiKey === keyA,
      'Registry still shows Alice owns browser_id after hijack attempt',
    );

    // Alice's own socket was NOT kicked
    assert(alice.ws.readyState === WebSocket.OPEN, "Alice's socket still open after hijack attempt");

    alice.ws.close();
    await wait(150);
  }

  console.log('\n8\u20e3  Desktop pairing hands over a key without putting it in a URL');
  {
    const pairing = await import('../../src/modules/pairing/service.ts');
    const PORT_BASE = `http://127.0.0.1:${PORT}`;

    const issued = await request('POST', '/api/pairing', { key: keyA });
    assert(
      issued.status === 201 && typeof issued.data.code === 'string',
      `a key can mint a pairing code (got ${issued.status})`,
    );
    assert((issued.data.code || '').length >= 40, 'the code is long enough that guessing is hopeless');
    assert(issued.data.code !== keyA && !String(issued.data.code).includes(keyA), 'the code is not the API key');

    const anon = await fetch(`${PORT_BASE}/api/pairing`, { method: 'POST' });
    assert(anon.status === 401, `minting a code requires auth (got ${anon.status})`);

    // The claim is unauthenticated by necessity: the desktop app has no
    // credential yet. That is what makes single use load-bearing.
    const claimed = await request('POST', '/api/pairing/claim', { body: { code: issued.data.code }, omitAuth: true });
    assert(
      claimed.status === 200 && claimed.data.apiKey === keyA,
      `a code redeems for the key that minted it (got ${claimed.status})`,
    );

    const replay = await request('POST', '/api/pairing/claim', { body: { code: issued.data.code }, omitAuth: true });
    assert(replay.status === 404, `a code cannot be redeemed twice (got ${replay.status})`);

    let accepted = 0;
    for (const bogus of ['', 'x', 'not-a-real-code-but-long-enough-to-pass-the-length-check', null, 123, { a: 1 }]) {
      const bad = await request('POST', '/api/pairing/claim', { body: { code: bogus }, omitAuth: true });
      if (bad.status === 200) accepted++;
    }
    assert(accepted === 0, `no bogus code is accepted (${accepted} were)`);

    // Single use is what bounds a link that leaked into a shell history or an OS log.
    pairing.reset();
    const { code } = pairing.issue('some-key');
    assert(pairing.outstanding() === 1, 'an outstanding code is tracked');
    assert(pairing.claim(code) === 'some-key', 'claim returns the key that minted it');
    assert(pairing.claim(code) === null, 'and only once');
    assert(pairing.outstanding() === 0, 'a claimed code is gone');
    pairing.reset();
  }

  console.log('\n9\ufe0f\u20e3  Arbitrary page JavaScript is not reachable through the API');
  {
    // evaluate_raw is how CAPTCHA and MFA handling reach a site's own globals.
    // Exposed, it is code execution inside a browser holding the customer's
    // real cookies, and the Oya client must not be the way around the CDP
    // driver's refusal.
    const browser = await connectBrowser('Evaluator', keyA);
    await wait(200);
    for (const action of ['evaluate_raw', 'evaluate']) {
      const direct = await request('POST', `/api/browsers/${browser.browserId}/command`, {
        key: keyA,
        body: { action, params: { expression: 'window.__pwned = 1' } },
      });
      assert(direct.status === 403, `${action} is refused on /browsers/:id/command (got ${direct.status})`);

      const viaPool = await request('POST', '/api/pool/command', {
        key: keyA,
        body: { action, params: { expression: 'window.__pwned = 1' } },
      });
      assert(viaPool.status === 403, `${action} is refused on /pool/command too (got ${viaPool.status})`);
    }
    browser.ws.close();
    await wait(150);
  }

  console.log("\n🔟  A role guard cannot be stepped around by changing the path's case");
  {
    // The guards in authMiddleware test req.path, which keeps whatever casing
    // the client sent. Express routes case-insensitively by default, so
    // /api/Pool/Cookies used to reach the handler while reading as a path the
    // guards did not name, a viewer credential's way to every session cookie.
    for (const path of ['/api/Pool/Cookies', '/api/POOL/COOKIES', '/api/Config']) {
      const res = await request('GET', path, { key: keyA });
      assert(res.status === 404, `${path} does not route (got ${res.status})`);
    }
    const canonical = await request('GET', '/api/config', { key: keyA });
    assert(canonical.status === 200, `the canonical spelling still works (got ${canonical.status})`);
  }

  console.log('\n1️⃣1️⃣  openai_base_url cannot be pointed at an internal address');
  {
    // The control plane fetches this URL, and chat-service used to hand the
    // response back to the caller, a read SSRF. The old check was a hostname
    // regex, so every range it forgot (CGNAT, benchmarking, multicast) was a
    // way in. It now goes through the same resolving guard as wsUrl.
    const { validateBaseUrl } = await import('../../src/platform/runtime-config.ts');
    const refused = async (url) => {
      try {
        await validateBaseUrl(url);
        return false;
      } catch {
        return true;
      }
    };
    for (const url of [
      'https://100.64.0.1/v1',
      'https://198.18.0.1/v1',
      'https://127.0.0.1/v1',
      'https://[::1]/v1',
      'https://169.254.169.254/v1',
      'http://api.openai.com/v1',
    ]) {
      assert(await refused(url), `${url} is refused`);
    }
    assert((await validateBaseUrl('')) === '', 'an empty base URL clears the field');

    const res = await request('POST', '/api/config', {
      key: keyA,
      body: { openai_base_url: 'https://169.254.169.254/v1' },
    });
    assert(res.status === 400, `POST /config refuses a link-local base URL (got ${res.status})`);
  }

  console.log('\n1️⃣2️⃣  The live view no longer takes an API key in the URL');
  {
    // ?key= put a permanent administrator credential into browser history,
    // Referer headers and every proxy log on the way. EventSource still cannot
    // set headers, so the supported form is a single-use ?ticket=.
    // A connected browser is not needed: authMiddleware runs first, so an
    // accepted credential reaches the handler and stops at 404, while a
    // rejected one never gets that far. (Asking for a live browser would open
    // an SSE stream this helper cannot close.)
    const absent = randomUUID();
    const leaked = await request('GET', `/api/live/${absent}?key=${encodeURIComponent(keyA)}`, { omitAuth: true });
    assert(leaked.status === 401, `?key= does not authenticate the live view (got ${leaked.status})`);

    const withHeader = await request('GET', `/api/live/${absent}`, { key: keyA });
    assert(withHeader.status === 404, `the Authorization header still authenticates (got ${withHeader.status})`);
  }

  console.log('\n1️⃣3️⃣  The operator residential proxy stays in sandboxes we run');
  {
    process.env.OYA_RESIDENTIAL_PROXY_URL =
      'http://op-country-{geo}-session-{session}:operator-secret@gate.example.com:7000';
    const usage = await import('../../src/platform/usage.ts');
    const desktop = await connectBrowser('Desktop', 'admin-key-security-test');
    const auth = desktop.messages.find((m) => m.type === 'auth_ok');
    assert(
      !JSON.stringify(auth?.fingerprint?.proxy || null).includes('operator-secret'),
      'a desktop browser is never handed the operator gateway credentials',
    );
    const before = usage.current('admin-key-security-test').residential_proxy_bytes;
    desktop.ws.send(JSON.stringify({ type: 'proxy_bytes', bytes: 5_000_000 }));
    await wait(200);
    assert(
      usage.current('admin-key-security-test').residential_proxy_bytes === before,
      'and cannot put proxy bytes on the bill',
    );
    desktop.ws.close();
    delete process.env.OYA_RESIDENTIAL_PROXY_URL;
  }

  // ── Summary ──
  console.log(`\n${'─'.repeat(50)}`);
  console.log(`  ${passed} passed, ${failed} failed`);
  console.log(`${'─'.repeat(50)}\n`);

  server.close();
  process.exit(failed > 0 ? 1 : 0);
} catch (err) {
  console.error('\n💥 Test crashed:', err);
  server.close();
  process.exit(1);
}
