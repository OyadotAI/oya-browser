#!/usr/bin/env node
/**
 * Control plane: metrics, usage accounting, audit trail, rate limits, quotas
 * and operator control actions.
 */

import { createServer } from 'http';
import express from 'express';
import { mkdtempSync } from 'fs';
import { tmpdir } from 'os';
import { join as joinPath } from 'path';

process.env.OYA_DATA_DIR = mkdtempSync(joinPath(tmpdir(), 'oya-test-'));
delete process.env.SUPABASE_URL;
delete process.env.SUPABASE_SERVICE_KEY;
process.env.API_KEYS = 'admin-key';
process.env.FLEET_TOKEN = 'tenant-key';
process.env.OYA_LIMIT_COMMANDS_PER_MIN = '60';
process.env.OYA_LIMIT_COMMANDS_BURST = '3';
process.env.OYA_METRICS_TOKEN = 'scrape-token';
process.env.OYA_OPERATOR_TOKEN = 'operator-token';
process.env.OYA_PROFILE_SECRET = 'b'.repeat(64); // read at module load

const { router } = await import('../../src/app/api.ts');
const { registry } = await import('../../src/modules/browsers/registry.ts');
const { metrics } = await import('../../src/platform/metrics.ts');
const usage = await import('../../src/platform/usage.ts');
const { recent } = await import('../../src/platform/audit.ts');

let passed = 0,
  failed = 0;
const assert = (c, label) => {
  if (c) {
    console.log(`  ✅ ${label}`);
    passed++;
  } else {
    console.log(`  ❌ ${label}`);
    failed++;
  }
};

const app = express();
app.use(express.json());
app.use('/api', router);
app.use('/', router);
const server = createServer(app);
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const base = `http://127.0.0.1:${server.address().port}`;

const call = async (path, { method = 'GET', body, key = 'admin-key', raw = false } = {}) => {
  const res = await fetch(base + path, {
    method,
    headers: { Authorization: `Bearer ${key}`, 'Content-Type': 'application/json' },
    ...(body ? { body: JSON.stringify(body) } : {}),
  });
  return {
    status: res.status,
    headers: res.headers,
    body: raw ? await res.text() : await res.json().catch(() => ({})),
  };
};

try {
  console.log('\n1️⃣  Metrics exposition...');
  metrics.commands.inc({ action: 'navigate', outcome: 'ok' });
  metrics.commandDuration.observe({ action: 'navigate' }, 42);

  const anon = await fetch(`${base}/metrics`);
  assert(anon.status === 403, `unauthenticated scrape is refused (got ${anon.status})`);
  const byApiKey = await call('/metrics', { key: 'admin-key', raw: true });
  assert(byApiKey.status === 403, 'an API key cannot scrape host metrics');
  const scraped = await call('/metrics', { key: 'operator-token', raw: true });
  assert(scraped.status === 200, 'the operator token can scrape');
  assert(/^# HELP /m.test(scraped.body), 'output is Prometheus text exposition');
  assert(/oya_commands_total\{action="navigate",outcome="ok"\} \d+/.test(scraped.body), 'counters carry their labels');
  assert(/oya_command_duration_ms_bucket\{.*le="50"\}/.test(scraped.body), 'histogram emits cumulative buckets');
  assert(/oya_event_loop_lag_p99_ms/.test(scraped.body), 'event loop health is exported');

  const fleet = await call('/api/fleet', { key: 'tenant-key' });
  assert(fleet.status === 200, 'any key can read its own fleet view — no admin tier');
  assert(typeof fleet.body.browsers.total === 'number', 'its browser count is reported');
  assert(fleet.body.routing && fleet.body.usage, 'routing and usage come with it');

  console.log('\n2️⃣  Cardinality is bounded...');
  // A per-browser label would be one series per browser at 5k. Prove the
  // registry refuses to grow without bound instead.
  for (let i = 0; i < 2500; i++) metrics.commands.inc({ action: `act${i}`, outcome: 'ok' });
  assert(metrics.commands.series.size <= 2000, `label explosion is capped (${metrics.commands.series.size} series)`);

  console.log('\n3️⃣  Rate limits enforce...');
  // A driver-backed browser answers immediately, which also exercises the
  // outbound dispatch path a CDP browser uses.
  registry.add('b-1', {
    apiKey: 'tenant-key',
    name: 'B1',
    clientType: 'cdp',
    provider: 'cdp',
    driver: { send: async () => ({ ok: true, data: {} }), close() {} },
  });
  const codes = [];
  for (let i = 0; i < 5; i++) {
    const r = await call('/api/browsers/b-1/command', { method: 'POST', body: { action: 'noop' }, key: 'tenant-key' });
    codes.push(r.status);
  }
  assert(codes.filter((c) => c === 429).length >= 2, `burst of 3 then 429 (got ${codes.join(',')})`);
  const limited = await call('/api/browsers/b-1/command', {
    method: 'POST',
    body: { action: 'noop' },
    key: 'tenant-key',
  });
  assert(limited.headers.get('retry-after'), 'a 429 carries Retry-After');
  assert(limited.headers.get('ratelimit-limit') === '60', 'RateLimit-Limit header is set');

  const otherKey = await call('/api/browsers/b-1/command', {
    method: 'POST',
    body: { action: 'noop' },
    key: 'admin-key',
  });
  assert(otherKey.status !== 429, "one key's limit does not affect another");

  console.log('\n4️⃣  Usage accounting...');
  usage.record('tenant-key', 'commands', 5);
  usage.record('tenant-key', 'chat_input_tokens', 1200);
  const mine = await call('/api/usage', { key: 'tenant-key' });
  assert(mine.status === 200, 'a tenant can read its own usage without admin rights');
  assert(mine.body.current.commands >= 5, 'commands are counted for the calling key');
  assert(mine.body.current.chat_input_tokens === 1200, 'model tokens are counted');
  assert(mine.body.limits.command.limit === 60, 'remaining allowance is reported back');
  assert(mine.body.current.rate_limited > 0, 'rate-limited requests are recorded against the key');

  const otherView = await call('/api/usage', { key: 'admin-key' });
  assert(otherView.body.current.commands === 0, "one key cannot see another key's usage");

  console.log('\n5️⃣  Audit trail...');
  await call('/api/pool/cookies', { method: 'DELETE', key: 'tenant-key' });
  const trail = await call('/api/audit', { key: 'tenant-key' });
  assert(trail.status === 200, 'any key can read its own audit trail');
  const clear = trail.body.events.find((e) => e.action === 'cookies.clear');
  assert(clear != null, 'clearing cookies is audited');
  assert(clear.actor && clear.actor !== 'tenant-key', 'the actor is a fingerprint, not the key');
  assert(clear.ip != null, 'the source address is recorded');

  const otherTrail = await call('/api/audit', { key: 'admin-key' });
  assert(
    !(otherTrail.body.events || []).some((e) => e.action === 'cookies.clear'),
    "one key does not see another key's audit events",
  );

  console.log('\n6️⃣  Operator control...');
  assert(
    (await call('/api/browsers/b-1/disconnect', { method: 'POST', key: 'admin-key' })).status === 404,
    "a key cannot disconnect another key's browser",
  );
  assert(
    (await call('/api/browsers/b-1/disconnect', { method: 'POST', key: 'tenant-key' })).status === 200,
    'a key can disconnect its own browser',
  );
  assert(!registry.isConnected('b-1'), 'the browser is gone from the registry');
  assert(recent({ action: 'browser.disconnect' }).length > 0, 'the disconnect is audited');

  const drain = await call('/api/operator/drain', { method: 'POST', body: { draining: true }, key: 'operator-token' });
  assert(drain.body.draining === true, 'the operator token can drain the host');
  assert(registry.draining === true, 'the WebSocket handler will see it');
  await call('/api/operator/drain', { method: 'POST', body: { draining: false }, key: 'operator-token' });

  assert(
    (await call('/api/operator/drain', { method: 'POST', key: 'tenant-key' })).status === 403,
    'an API key cannot drain the host',
  );

  console.log('\n7️⃣  Providers...');
  const providers = await call('/api/providers', { key: 'tenant-key' });
  assert(
    providers.body.providers.some((p) => p.name === 'cdp' && p.configured),
    'the bring-your-own-CDP provider is always available',
  );
  assert(
    providers.body.providers.some((p) => p.name === 'anchor'),
    'hosted providers are listed with their config state',
  );

  const badConnect = await call('/api/browsers/connect', {
    method: 'POST',
    body: { provider: 'cdp' },
    key: 'tenant-key',
  });
  assert(badConnect.status === 400, 'connecting without a wsUrl is rejected');
  console.log('\n8.  The host does not dial caller-supplied private addresses...');
  const { assertSafeTarget } = await import('../../src/platform/net-guard.ts');
  delete process.env.OYA_ALLOW_PRIVATE_TARGETS;
  const hostile = [
    'ws://127.0.0.1:6379/',
    'ws://169.254.169.254/',
    'wss://[::1]/x',
    'ws://10.1.2.3/x',
    'ws://user:pw@example.com/',
    'http://example.com/',
    'garbage',
  ];
  let blocked = 0;
  for (const url of hostile) {
    try {
      await assertSafeTarget(url, { label: 'wsUrl' });
    } catch (e) {
      if (e.status === 400) blocked++;
    }
  }
  assert(blocked === hostile.length, `all ${hostile.length} unsafe targets refused (${blocked})`);

  const viaApi = await call('/api/gateway/providers', {
    method: 'POST',
    key: 'tenant-key',
    body: { name: 'ssrf', type: 'cdp', wsUrl: 'ws://169.254.169.254/' },
  });
  assert(viaApi.status === 400, `provider registration refuses a metadata address (got ${viaApi.status})`);

  process.env.OYA_ALLOW_PRIVATE_TARGETS = 'true';
  const allowed = await assertSafeTarget('ws://127.0.0.1:9222/x', { label: 'wsUrl' }).then(
    () => true,
    () => false,
  );
  assert(allowed, 'a single-operator host can opt in to loopback targets');
  // The escape hatch must not cover cloud metadata: opting into loopback for a
  // laptop should never open 169.254.169.254 on a cloud VM.
  const stillBlocked = [];
  for (const url of ['ws://169.254.169.254/', 'wss://[fe80::1]/x', 'ws://0.0.0.0/x']) {
    const ok = await assertSafeTarget(url, { label: 'wsUrl' }).then(
      () => true,
      () => false,
    );
    if (!ok) stillBlocked.push(url);
  }
  assert(stillBlocked.length === 3, 'link-local and reserved stay blocked even with the opt-in on');
  delete process.env.OYA_ALLOW_PRIVATE_TARGETS;

  console.log('\n9.  Operator token is header-only...');
  const viaQuery = await fetch(`${base}/metrics?token=operator-token`);
  assert(viaQuery.status === 403, `a token in the query string is not accepted (got ${viaQuery.status})`);
  assert((await call('/metrics', { key: 'operator-token', raw: true })).status === 200, 'the header is');

  console.log('\n10. Routing strategy is per key...');
  await call('/api/gateway/strategy', { method: 'POST', key: 'tenant-key', body: { strategy: 'latency' } });
  const mineStrategy = await call('/api/gateway/providers', { key: 'tenant-key' });
  const theirStrategy = await call('/api/gateway/providers', { key: 'admin-key' });
  assert(mineStrategy.body.strategy === 'latency', "the key's own strategy changed");
  assert(theirStrategy.body.strategy !== 'latency', "another key's routing is unaffected");
  console.log('\n11. Personas — the identity unit...');
  const P = (await import('../../src/app/container.ts')).container.personas;

  // 1. Distinct personas get distinct fingerprints. Before this, 1,000
  //    browsers on one key shared one identity and one canvas hash.
  const made = Array.from({ length: 20 }, (_, i) => P.create('tenant-key', { name: `p${i}` }));
  const seeds = new Set(made.map((p) => P.fingerprintFor(p).canvas.noiseSeed));
  assert(seeds.size === 20, `20 personas produce 20 distinct fingerprints (got ${seeds.size})`);

  // 2. One persona is STABLE. This is the property that keeps the fingerprint
  //    coherent with the cookies, and the one a per-browser seed would break.
  const before = JSON.stringify(P.fingerprintFor(made[0]));
  const again = JSON.stringify(P.fingerprintFor(P.get('tenant-key', made[0].id)));
  assert(before === again, 'the same persona yields a byte-identical fingerprint');

  // 3. The default persona reproduces the pre-persona fingerprint exactly, so
  //    a customer running one account sees no change.
  const { getFingerprintForKey } = await import('../../src/modules/personas/fingerprint.ts');
  assert(
    JSON.stringify(P.fingerprintFor(P.defaultFor('tenant-key'))) === JSON.stringify(getFingerprintForKey('tenant-key')),
    'the default persona matches the fingerprint that key had before personas',
  );

  // 4. Concurrency is capped, because one device cannot be in many places.
  const capped = P.create('tenant-key', { name: 'capped', maxConcurrent: 2 });
  P.acquire(capped, 'b1');
  P.acquire(capped, 'b2');
  let refused = false;
  try {
    P.acquire(capped, 'b3');
  } catch (e) {
    refused = e.status === 429;
  }
  assert(refused, 'a third browser on a cap of 2 is refused');
  P.release(capped, 'b1');
  assert(P.acquire(capped, 'b3') === capped, 'releasing a slot lets the next one in');
  assert(P.activeCount(capped.id) === 2, 'active count tracks reality');

  // 5. Ownership is the boundary — a persona id must not be usable by another
  //    key, or one customer drives another's logged-in sessions.
  assert(P.get('admin-key', made[0].id) === null, "another key cannot resolve someone else's persona");
  let denied = false;
  try {
    P.resolve('admin-key', made[0].id);
  } catch (e) {
    denied = e.status === 404;
  }
  assert(denied, "resolving another key's persona is refused");
  assert(
    P.list('admin-key').every((p) => p.id !== made[0].id),
    'and it is not listed',
  );

  // 6. The default persona is permanent; a busy one cannot be deleted.
  let guarded = false;
  try {
    P.remove('tenant-key', P.defaultFor('tenant-key').id);
  } catch (e) {
    guarded = e.status === 400;
  }
  assert(guarded, 'the default persona cannot be deleted');
  let busy = false;
  try {
    P.remove('tenant-key', capped.id);
  } catch (e) {
    busy = e.status === 409;
  }
  assert(busy, 'a persona with running browsers cannot be deleted');

  console.log('\n12. Personas over the API...');
  const created = await call('/api/personas', { method: 'POST', key: 'tenant-key', body: { name: 'via-api' } });
  assert(created.status === 201 && created.body.id, 'a persona can be created over the API');
  assert(created.body.fingerprint?.canvasSeed !== undefined, 'its fingerprint is reported');
  assert(created.body.seed === undefined, 'the seed is never exposed');
  const mineList = await call('/api/personas', { key: 'tenant-key' });
  assert(
    mineList.body.personas.some((p) => p.id === created.body.id),
    'it is listed for its owner',
  );
  const theirs = await call('/api/personas', { key: 'admin-key' });
  assert(!theirs.body.personas.some((p) => p.id === created.body.id), 'and not for anyone else');
  assert(
    (await call(`/api/personas/${created.body.id}`, { key: 'admin-key' })).status === 404,
    "another key gets 404, not another key's persona",
  );

  console.log('\n13. Proxies — an identity needs its own exit...');
  const X = await import('../../src/modules/proxies/service.ts');
  const ownerFp = (await import('../../src/platform/audit.ts')).fingerprint('tenant-key');

  const px = await X.register({ owner: ownerFp, label: 'us-1', url: 'http://u:p@example.com:8080', geo: 'US' });
  assert(px.id.startsWith('px-'), 'a proxy can be registered');
  assert(
    X.list(ownerFp).some((p) => p.id === px.id),
    'and listed for its owner',
  );
  assert(
    X.list((await import('../../src/platform/audit.ts')).fingerprint('admin-key')).every((p) => p.id !== px.id),
    'but not for another key',
  );

  // Credentials are the whole reason this is encrypted at rest.
  const shown = JSON.stringify(px.toJSON());
  assert(!shown.includes('example.com') && !shown.includes('u:p'), 'the API view carries no host or credentials');
  assert(X.credentials(px).url === 'http://example.com:8080', 'the server can still decrypt them');
  assert(X.credentials(px).username === 'u', 'including the username');

  // Chromium silently drops SOCKS5 auth, and that is what residential vendors
  // sell — refusing beats handing back an exit that does not apply.
  let socksRefused = false;
  try {
    await X.register({ owner: ownerFp, url: 'socks5://user:pw@example.com:1080' });
  } catch (e) {
    socksRefused = e.status === 400 && /SOCKS5/.test(e.message);
  }
  assert(socksRefused, 'an authenticated SOCKS5 proxy is refused with the reason');

  // Stickiness: a persona keeps its exit, or a returning login looks stolen.
  const persona = P.create('tenant-key', { name: 'proxied' });
  const first = X.forPersona(ownerFp, persona);
  assert(first?.id === px.id, 'a persona is assigned a proxy');
  assert(X.forPersona(ownerFp, persona)?.id === px.id, 'and keeps the same one');

  // Capacity is per proxy, so two personas do not silently share one exit.
  const second = P.create('tenant-key', { name: 'proxied-2' });
  assert(X.forPersona(ownerFp, second) === null, 'a proxy at capacity is not handed to a second persona');

  // Timezone that contradicts the exit country is a cheap detection.
  const fp = P.fingerprintFor(persona);
  const bad = X.coherence(persona, { ...fp, timezone: 'Europe/Berlin' }, { ...px, geo: 'US' });
  assert(bad.checked && bad.ok === false, 'a Berlin timezone behind a US exit is flagged');
  const good = X.coherence(persona, { ...fp, timezone: 'America/Denver' }, { ...px, geo: 'US' });
  assert(good.checked && good.ok === true, 'a Denver timezone behind a US exit is fine');

  // Out of the box: the operator's residential gateway, one sticky session per persona.
  process.env.OYA_RESIDENTIAL_PROXY_URL = 'http://cust-country-{geo}-session-{session}:pw@gate.example.com:7000';
  const r1 = X.residential(persona),
    r2 = X.residential(persona),
    r3 = X.residential(second);
  assert(
    r1.url === 'http://gate.example.com:7000' && r1.password === 'pw',
    'the residential gateway is used with no proxy registered',
  );
  assert(r1.username === r2.username && r1.username !== r3.username, 'each persona keeps its own sticky session');
  assert(
    /^cust-country-us-session-[0-9a-f]{16}$/.test(r1.username),
    `and exits in its country, US by default (got ${r1.username})`,
  );
  const de = P.create('tenant-key', { name: 'berlin', proxy: { geo: 'DE' } });
  assert(X.residential(de).username.includes('country-de-'), 'a persona geo hint picks the country');
  delete process.env.OYA_RESIDENTIAL_PROXY_URL;
  assert(X.residential(persona) === null, 'unset, browsers go direct as before');

  // The check tunnels through the proxy with the standard library and learns the exit IP.
  // It once imported a package the server does not ship, so every check failed and
  // put working proxies into cooldown.
  {
    const net = await import('net');
    const http = await import('http');
    const ipSite = http.createServer((_q, s) => s.end(JSON.stringify({ ip: '203.0.113.9' })));
    await new Promise((r) => ipSite.listen(0, '127.0.0.1', r));
    let sawAuth = '';
    const tunnel = http.createServer();
    tunnel.on('connect', (req, sock) => {
      sawAuth = req.headers['proxy-authorization'] || '';
      const [host, port] = req.url.split(':');
      const up = net.connect(Number(port), host, () => {
        sock.write('HTTP/1.1 200 Connection Established\r\n\r\n');
        up.pipe(sock);
        sock.pipe(up);
      });
      up.on('error', () => sock.destroy());
    });
    await new Promise((r) => tunnel.listen(0, '127.0.0.1', r));
    process.env.OYA_ALLOW_PRIVATE_TARGETS = 'true';
    process.env.OYA_PROXY_CHECK_URL = `http://127.0.0.1:${ipSite.address().port}/`;
    const local = await X.register({
      owner: ownerFp,
      label: 'local',
      url: `http://cu:cp@127.0.0.1:${tunnel.address().port}`,
    });
    const checked = await X.check(local);
    assert(
      checked.ok && checked.exitIp === '203.0.113.9',
      `a proxy check learns the exit IP through the tunnel (got ${JSON.stringify(checked)})`,
    );
    assert(sawAuth === `Basic ${Buffer.from('cu:cp').toString('base64')}`, 'and authenticates to the proxy');
    X.remove(ownerFp, local.id);
    tunnel.close();
    ipSite.close();
    delete process.env.OYA_ALLOW_PRIVATE_TARGETS;
    delete process.env.OYA_PROXY_CHECK_URL;
  }

  console.log('\n14. Proxies over the API...');
  const proxyViaApi = await call('/api/proxies', {
    method: 'POST',
    key: 'tenant-key',
    body: { label: 'api', url: 'http://a:b@example.com:3128', geo: 'DE' },
  });
  assert(proxyViaApi.status === 201, 'a proxy can be added over the API');
  assert(!JSON.stringify(proxyViaApi.body).includes('example.com'), 'the response leaks no host');
  const badSocks = await call('/api/proxies', {
    method: 'POST',
    key: 'tenant-key',
    body: { url: 'socks5://u:p@h:1080' },
  });
  assert(badSocks.status === 400, 'the SOCKS5 auth limitation is surfaced over the API too');

  console.log('\n15. Settings follow the API key...');
  {
    const keyConfig = await import('../../src/modules/config/service.ts');

    const saved = await call('/api/config', {
      method: 'POST',
      key: 'tenant-key',
      body: { browser_provider: 'steel', steel_api_key: 'steel-secret-9999', chat_model: 'tenant-model' },
    });
    assert(saved.status === 200, `a key stores its own settings (got ${saved.status})`);
    assert(saved.body.steel_api_key === '••••9999', 'a stored provider credential comes back masked');

    const mine = await call('/api/config', { key: 'tenant-key' });
    assert(mine.body.browser_provider === 'steel', 'the key reads back its own provider');
    assert(!JSON.stringify(mine.body).includes('steel-secret-9999'), 'no clear credential in the response');

    const other = await call('/api/config', { key: 'admin-key' });
    assert(other.body.browser_provider !== 'steel', 'another key sees none of it');

    // This is what makes the whole thing per-key: providers.js reads an env
    // object, so a key's credentials become that key's environment.
    assert(
      keyConfig.envFor('tenant-key').STEEL_API_KEY === 'steel-secret-9999',
      "envFor layers the key's provider credential over process.env",
    );
    assert(keyConfig.envFor('admin-key').STEEL_API_KEY === undefined, "another key's environment is untouched");

    assert(keyConfig.providerFor('tenant-key') === 'steel', 'providerFor reads the stored provider');
    assert(keyConfig.providerFor('admin-key') === 'cdp', 'a key that set nothing falls back to cdp');

    // Only the operator can move the deployment-wide default.
    const hostByTenant = await call('/api/config/host', {
      method: 'POST',
      key: 'tenant-key',
      body: { chat_model: 'hijacked' },
    });
    assert(hostByTenant.status === 403, `an API key cannot write the host default (got ${hostByTenant.status})`);
  }

  console.log('\n16. POST /browsers/start hides the provider...');
  {
    const noAuth = await fetch(`${base}/api/browsers/start`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: '{}',
    });
    assert(noAuth.status === 401, `start requires auth (got ${noAuth.status})`);

    // tenant-key's stored provider is steel, with a credential that is not real:
    // reaching the vendor at all proves the key's own settings drove the call.
    const viaSettings = await call('/api/browsers/start', { method: 'POST', key: 'tenant-key' });
    assert(
      viaSettings.status === 502 || viaSettings.status === 409,
      `the stored provider is used without the caller naming it (got ${viaSettings.status})`,
    );
    assert(!/cdp/.test(viaSettings.body.error || ''), 'it did not silently fall back to cdp');

    // An explicit provider still wins, and cdp still needs a URL.
    const explicit = await call('/api/browsers/start', {
      method: 'POST',
      key: 'tenant-key',
      body: { provider: 'cdp' },
    });
    assert(
      explicit.status === 400 && /wsUrl/.test(explicit.body.error || ''),
      'an explicit provider overrides the stored one',
    );

    // A wsUrl pointing inward is refused before anything is dialled.
    const ssrf = await call('/api/browsers/start', {
      method: 'POST',
      key: 'tenant-key',
      body: { provider: 'cdp', wsUrl: 'ws://169.254.169.254/devtools/browser/x' },
    });
    assert(ssrf.status >= 400, `start refuses an internal wsUrl (got ${ssrf.status})`);

    const unknownPersona = await call('/api/browsers/start', {
      method: 'POST',
      key: 'tenant-key',
      body: { provider: 'cdp', wsUrl: 'ws://example.com:9222/x', persona: 'not-mine' },
    });
    assert(unknownPersona.status === 404, `an unowned persona is refused (got ${unknownPersona.status})`);
  }

  console.log('\n17. The fleet console: activity, health, stop...');
  {
    let released = 0;
    registry.add('fc-1', {
      apiKey: 'tenant-key',
      name: 'Console 1',
      clientType: 'cdp',
      provider: 'cdp',
      driver: {
        send: async (action) => (action === 'boom' ? { ok: false, error: 'nope' } : { ok: true, data: {} }),
        close() {},
      },
      release: () => {
        released++;
      },
    });
    // Rate limits from section 3 are per key; use the admin key for volume.
    registry.add('fc-2', {
      apiKey: 'admin-key',
      name: 'Console 2',
      clientType: 'cdp',
      provider: 'cdp',
      driver: {
        send: async (action) => (action === 'boom' ? { ok: false, error: 'nope' } : { ok: true, data: {} }),
        close() {},
      },
    });
    const { sendCommand } = await import('../../src/modules/browsers/socket.ts');
    await sendCommand('fc-2', 'navigate', { url: 'https://example.com/a' });
    await sendCommand('fc-2', 'type', { selector: '#q', text: 'my secret password' });
    await sendCommand('fc-2', 'boom', {});

    const one = await call('/api/browsers/fc-2', { key: 'admin-key' });
    assert(one.status === 200, `GET /browsers/:id answers (got ${one.status})`);
    assert(
      one.body.commands === 3 && one.body.errors === 1,
      `counters: ${one.body.commands} commands, ${one.body.errors} errors`,
    );
    assert(Array.isArray(one.body.activity) && one.body.activity.length === 3, 'activity holds the three commands');
    assert(one.body.activity[0].action === 'boom' && one.body.activity[0].ok === false, 'newest first, failure marked');
    assert(one.body.activity[2].summary === 'https://example.com/a', 'navigate summary is the url');
    assert(!JSON.stringify(one.body.activity).includes('my secret password'), 'typed text never enters the log');
    assert(one.body.health === 'ok', `health is derived (${one.body.health})`);
    const other = await call('/api/browsers/fc-2', { key: 'tenant-key' });
    assert(other.status === 404, "another key cannot read a browser's activity");

    const listed = await call('/api/browsers', { key: 'admin-key' });
    const row = listed.body.find((b) => b.id === 'fc-2');
    assert(row && row.health === 'ok' && row.commands === 3, 'the list carries health and counters');
    const fleet = await call('/api/fleet', { key: 'admin-key' });
    assert(
      fleet.body.browsers.byHealth.ok >= 1 && fleet.body.browsers.commands >= 3,
      'fleet rolls up health and command counts',
    );

    const stop = await call('/api/browsers/fc-1/stop', { method: 'POST', key: 'tenant-key' });
    assert(stop.status === 200 && stop.body.ok, `stop answers (got ${stop.status})`);
    assert(released === 1, 'stopping a CDP browser releases its vendor session');
    assert(!registry.isConnected('fc-1'), 'and removes it from the registry');
    const gone = await call('/api/browsers/fc-1/stop', { method: 'POST', key: 'tenant-key' });
    assert(gone.status === 404, 'stopping it again is a 404, not a silent ok');

    registry.add('fc-3', {
      apiKey: 'admin-key',
      name: 'C3',
      clientType: 'cdp',
      provider: 'cdp',
      driver: { send: async () => ({ ok: true }), close() {} },
    });
    registry.add('fc-4', {
      apiKey: 'admin-key',
      name: 'C4',
      clientType: 'cdp',
      provider: 'cdp',
      driver: { send: async () => ({ ok: true }), close() {} },
    });
    registry.add('fc-5', {
      apiKey: 'tenant-key',
      name: 'C5',
      clientType: 'cdp',
      provider: 'cdp',
      driver: { send: async () => ({ ok: true }), close() {} },
    });
    const bulk = await call('/api/browsers/stop', {
      method: 'POST',
      key: 'admin-key',
      body: { ids: ['fc-3', 'fc-4', 'fc-5'] },
    });
    assert(bulk.body.stopped === 2, `bulk stop stops only the caller's browsers (${bulk.body.stopped} of 3)`);
    assert(registry.isConnected('fc-5'), "another key's browser survives a bulk stop by id");
    const all = await call('/api/browsers/stop', { method: 'POST', key: 'tenant-key', body: { all: true } });
    assert(all.body.stopped >= 1 && !registry.isConnected('fc-5'), 'all: true stops everything on that key');
    const empty = await call('/api/browsers/stop', { method: 'POST', key: 'tenant-key', body: {} });
    assert(empty.status === 400, 'stop without ids or all is a 400');
  }

  console.log('\n18. Personas choose a device at creation, and keep it...');
  {
    const opts = await call('/api/personas/options', { key: 'tenant-key' });
    assert(
      Array.isArray(opts.body.platforms) && opts.body.timezones.MacIntel,
      'options list platforms and per-platform timezones',
    );

    const before = P.list('tenant-key').length;
    const pv = await call('/api/personas/preview', {
      method: 'POST',
      key: 'tenant-key',
      body: { prefs: { platform: 'MacIntel', timezone: 'Pacific/Honolulu' } },
    });
    assert(
      pv.body.fingerprint?.platform === 'MacIntel' && pv.body.fingerprint?.timezone === 'Pacific/Honolulu',
      'preview honours the choices',
    );
    assert(P.list('tenant-key').length === before, 'preview persists nothing');

    // A choice that cannot be honoured is refused, never swapped for another device.
    const refused = await call('/api/personas', {
      method: 'POST',
      key: 'tenant-key',
      body: { name: 'nowhere', prefs: { platform: 'Win32', timezone: 'Mars/Olympus' } },
    });
    assert(
      refused.status === 400 && /timezone/.test(refused.body.error),
      'an unknown timezone is refused, with the reason',
    );
    const berlin = await call('/api/personas/preview', {
      method: 'POST',
      key: 'tenant-key',
      body: { prefs: { platform: 'Win32', timezone: 'Europe/Berlin', locale: 'de-DE' } },
    });
    assert(
      berlin.body.fingerprint?.timezone === 'Europe/Berlin' && berlin.body.fingerprint?.locale === 'de-DE',
      'a Windows machine in Berlin is honoured',
    );
    // Personas made before choices were validated keep the device they were made as.
    const { previewProfile } = await import('../../src/modules/personas/fingerprint.ts');
    const legacy = previewProfile({ id: 'legacy', seed: 42, prefs: { platform: 'Win32', timezone: 'Europe/Berlin' } });
    assert(
      legacy.timezone !== 'Europe/Berlin' && legacy.webglChrome === false,
      'an older persona keeps its seeded timezone and its WebGL strings',
    );

    const created = await call('/api/personas', {
      method: 'POST',
      key: 'tenant-key',
      body: {
        name: 'Mac in Hawaii',
        prefs: { platform: 'MacIntel', timezone: 'Pacific/Honolulu', locale: 'en-GB' },
        maxConcurrent: 3,
      },
    });
    assert(created.status === 201, `create with prefs (got ${created.status})`);
    assert(
      created.body.fingerprint.platform === 'MacIntel' &&
        created.body.fingerprint.timezone === 'Pacific/Honolulu' &&
        created.body.fingerprint.locale === 'en-GB',
      'the created persona is the previewed device',
    );
    assert(created.body.prefs?.platform === 'MacIntel', 'prefs are returned');
    assert(created.body.mfa && created.body.mfa.configured === false, 'describe includes MFA state');
    const pid = created.body.id;

    // The device must survive a restart: prefs are stored with the seed.
    const seedBefore = P.get('tenant-key', pid).seed;
    await P.drain();
    P.reset();
    await P.restore();
    const back = P.get('tenant-key', pid);
    assert(
      back && back.seed === seedBefore && back.prefs?.timezone === 'Pacific/Honolulu',
      'seed and prefs survive drain/restore',
    );

    const bad = await call('/api/personas/' + pid, {
      method: 'PUT',
      key: 'tenant-key',
      body: { prefs: { platform: 'Win32' } },
    });
    assert(
      bad.status === 400 && /clone/i.test(bad.body.error),
      'changing prefs after creation is refused, with the way out named',
    );
    const badSeed = await call('/api/personas/' + pid, { method: 'PUT', key: 'tenant-key', body: { seed: 1 } });
    assert(badSeed.status === 400, 'so is changing the seed');

    const renamed = await call('/api/personas/' + pid, {
      method: 'PUT',
      key: 'tenant-key',
      body: { name: 'Renamed', maxConcurrent: 5 },
    });
    assert(
      renamed.status === 200 && renamed.body.name === 'Renamed' && renamed.body.maxConcurrent === 5,
      'name and cap are editable',
    );
    assert(renamed.body.fingerprint.timezone === 'Pacific/Honolulu', 'and the device did not move');
    const foreign = await call('/api/personas/' + pid, { method: 'PUT', key: 'admin-key', body: { name: 'x' } });
    assert(foreign.status === 404, 'another key cannot edit it');

    const cloned = await call('/api/personas/' + pid + '/clone', { method: 'POST', key: 'tenant-key', body: {} });
    assert(cloned.status === 201 && cloned.body.id !== pid, 'clone makes a new persona');
    assert(
      cloned.body.fingerprint.platform === 'MacIntel' && cloned.body.fingerprint.timezone === 'Pacific/Honolulu',
      'of the same kind of device',
    );
    assert(cloned.body.fingerprint.canvasSeed !== created.body.fingerprint.canvasSeed, 'but a different device');

    const unpin = await call('/api/personas/' + pid + '/proxy', {
      method: 'PUT',
      key: 'tenant-key',
      body: { proxyId: null },
    });
    assert(unpin.status === 200 && unpin.body.proxy === null, 'a persona can be unpinned from its proxy');
    const nope = await call('/api/personas/' + pid + '/proxy', {
      method: 'PUT',
      key: 'tenant-key',
      body: { proxyId: 'not-a-proxy' },
    });
    assert(nope.status === 404, 'pinning to an unknown proxy is a 404');
  }
} catch (e) {
  console.log(`  ❌ threw: ${e.message}\n${e.stack}`);
  failed++;
} finally {
  await new Promise((r) => server.close(r));
}

console.log('\n──────────────────────────────────────────────────');
console.log(`  ${passed} passed, ${failed} failed`);
console.log('──────────────────────────────────────────────────');
process.exit(failed ? 1 : 0);
