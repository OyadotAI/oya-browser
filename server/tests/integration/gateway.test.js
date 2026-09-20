#!/usr/bin/env node
/**
 * CDP gateway end to end against a real Chrome.
 *
 * Proves the thing that matters: a plain CDP client (what Playwright,
 * Puppeteer, Stagehand and browser-use all are underneath) can point at this
 * control plane and drive a browser, with routing, profiles, reconnection and
 * recording layered on without the client knowing.
 */

import { createServer } from 'http';
import express from 'express';
import { spawn } from 'child_process';
import { mkdtempSync, existsSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { WebSocket } from 'ws';
import { createHash } from 'crypto';
import { removeScratch } from '../support/scratch.js';

const CHROME = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
].find((p) => existsSync(p));
if (!CHROME) {
  console.log('⏭  No Chrome binary — skipping gateway test');
  process.exit(0);
}

const DATA = mkdtempSync(join(tmpdir(), 'oya-gw-'));
process.env.OYA_DATA_DIR = DATA;
delete process.env.SUPABASE_URL;
delete process.env.SUPABASE_SERVICE_KEY;
process.env.API_KEYS = 'admin-key';
process.env.FLEET_TOKEN = 'tenant-key';
process.env.OYA_PROFILE_SECRET = 'a'.repeat(64);
process.env.OYA_SESSION_GRACE_MS = '10000';
process.env.OYA_RECORD_EVERY_NTH = '1';

const { handleJsonVersion, handleJsonList, handleUpgrade, sessions } =
  await import('../../src/modules/gateway/service.ts');
const { pool } = await import('../../src/modules/gateway/routing.ts');
const { CDPConnection } = await import('../../src/drivers/cdp.ts');
const profiles = await import('../../src/modules/gateway/profiles.ts');
const recorder = await import('../../src/modules/gateway/recorder.ts');

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
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

// ── A page to drive, and a real Chrome to drive it with ──
const site = createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end('<!doctype html><title>Gateway fixture</title><h1>hello</h1><div style="height:2000px"></div>');
});
await new Promise((r) => site.listen(0, '127.0.0.1', r));
const siteUrl = `http://127.0.0.1:${site.address().port}/`;

const profile = join(DATA, 'chrome');
const chrome = spawn(
  CHROME,
  ['--headless=new', '--remote-debugging-port=0', '--no-first-run', `--user-data-dir=${profile}`, 'about:blank'],
  { stdio: ['ignore', 'ignore', 'pipe'] },
);
const chromeWs = await new Promise((resolve, reject) => {
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

pool.register({ name: 'local-chrome', type: 'cdp', wsUrl: chromeWs, maxConcurrent: 4, priority: 1 });

const app = express();
app.use(express.json());
app.get('/json/version', handleJsonVersion);
app.get('/json/list', handleJsonList);
const server = createServer(app);
server.on('upgrade', (req, socket, head) => {
  if (new URL(req.url, 'http://x').pathname === '/connect') {
    handleUpgrade(req, socket, head).catch(() => socket.destroy());
  } else socket.destroy();
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const origin = `127.0.0.1:${server.address().port}`;
const gwUrl = (qs = '') => `ws://${origin}/connect?token=tenant-key${qs}`;

/** Minimal CDP client — the same thing Playwright is underneath. */
async function client(qs = '') {
  const conn = await new CDPConnection(gwUrl(qs)).connect();
  const { targetInfos = [] } = await conn.send('Target.getTargets');
  let page = targetInfos.find((t) => t.type === 'page');
  if (!page) page = { targetId: (await conn.send('Target.createTarget', { url: 'about:blank' })).targetId };
  const { sessionId } = await conn.send('Target.attachToTarget', { targetId: page.targetId, flatten: true });
  await conn.send('Page.enable', {}, sessionId).catch(() => {});
  await conn.send('Runtime.enable', {}, sessionId).catch(() => {});
  return { conn, sessionId, targetId: page.targetId };
}
const evaluate = (c, expr) =>
  c.conn
    .send('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true }, c.sessionId)
    .then((r) => r.result?.value);

/**
 * All sessions in this test share one Chrome, so browser state carries over
 * between them. Any assertion about what a profile restored has to start from
 * a clean browser or it passes for the wrong reason.
 */
async function wipeBrowser() {
  const c = await client();
  await c.conn.send('Network.clearBrowserCookies', {}, c.sessionId);
  await c.conn.send('Page.navigate', { url: siteUrl }, c.sessionId);
  await wait(500);
  await evaluate(c, 'localStorage.clear(); sessionStorage.clear(); true');
  const id = [...sessions.values()].filter((s) => s.client).slice(-1)[0].id;
  c.conn.close();
  await wait(200);
  await sessions.get(id)?.destroy('wipe');
  await wait(200);
}

try {
  console.log('\n1️⃣  CDP discovery — this is what makes clients work unchanged...');
  const version = await (await fetch(`http://${origin}/json/version`)).json();
  assert(version['Protocol-Version'] === '1.3', 'reports a CDP protocol version');
  assert(/^Chrome\//.test(version.Browser), `identifies as a browser (${version.Browser})`);
  assert(version.webSocketDebuggerUrl?.includes('/connect'), 'points clients at the gateway socket');

  console.log('\n2️⃣  Auth...');
  const rejected = await new Promise((resolve) => {
    const bad = new WebSocket(`ws://${origin}/connect?token=nope`);
    bad.on('unexpected-response', (_, res) => resolve(res.statusCode));
    bad.on('error', () => resolve('error'));
    bad.on('open', () => {
      bad.close();
      resolve('opened');
    });
  });
  assert(rejected === 401, `an invalid token is refused at upgrade (got ${rejected})`);

  console.log('\n3️⃣  Drive a real browser through the gateway...');
  const c1 = await client();
  assert(sessions.size === 1, 'the gateway opened one session');
  const nav = await c1.conn.send('Page.navigate', { url: siteUrl }, c1.sessionId);
  assert(nav.frameId, 'Page.navigate is forwarded to the real browser');
  await wait(600);
  assert((await evaluate(c1, 'document.title')) === 'Gateway fixture', 'the page really loaded');
  assert(
    (await evaluate(c1, 'document.querySelector("h1").textContent')) === 'hello',
    'DOM is reachable through the pipe',
  );

  const shot = await c1.conn.send('Page.captureScreenshot', { format: 'jpeg', quality: 40 }, c1.sessionId);
  assert(shot.data?.length > 1000, 'binary-ish payloads survive the pipe (screenshot)');

  const routed = pool.get(null, 'local-chrome'); // shared host provider
  assert(routed.active === 1, 'the provider shows one active session');
  assert(routed.totalSessions === 1, 'the routing pool counted it');

  console.log('\n4️⃣  Profiles persist a login across sessions...');
  const p1 = await client('&profile=acme');
  await p1.conn.send('Page.navigate', { url: siteUrl }, p1.sessionId);
  await wait(600);
  await evaluate(p1, `document.cookie = "sid=profile-value; path=/"; localStorage.setItem('who','acme'); true`);

  const sessionId = [...sessions.values()].find((s) => s.profile === 'acme').id;
  p1.conn.close();
  await wait(300);
  await sessions.get(sessionId)?.destroy('test'); // capture on end
  await wait(400);

  await wipeBrowser();
  const p2 = await client('&profile=acme');
  await p2.conn.send('Page.navigate', { url: siteUrl }, p2.sessionId);
  await wait(800);
  const cookie = await evaluate(p2, 'document.cookie');
  assert(/sid=profile-value/.test(cookie || ''), `the cookie came back in a new session (got "${cookie}")`);
  assert((await evaluate(p2, `localStorage.getItem('who')`)) === 'acme', 'localStorage came back too');

  console.log('\n5️⃣  A profile cannot be used twice at once...');
  const conflict = await new Promise((resolve) => {
    const w = new WebSocket(gwUrl('&profile=acme'));
    w.on('unexpected-response', (_, res) => resolve(res.statusCode));
    w.on('error', () => resolve('error'));
    w.on('open', () => {
      w.close();
      resolve('opened');
    });
  });
  assert(conflict === 409, `a concurrent connect to the same profile is refused with 409 (got ${conflict})`);
  // Release the lock properly: closing the client only holds the session for
  // resume, and the profile stays locked for that whole grace window.
  const p2Id = [...sessions.values()].find((s) => s.profile === 'acme')?.id;
  p2.conn.close();
  await wait(200);
  if (p2Id) await sessions.get(p2Id)?.destroy('profile test done');
  await wait(200);

  console.log('\n6️⃣  Encryption is bound to the profile name...');
  const sealed = profiles._internals.seal('acme', { secret: 'top' });
  assert(!sealed.toString('utf8').includes('top'), 'stored bytes do not contain the plaintext');
  assert(profiles._internals.open('acme', sealed).secret === 'top', 'it opens under its own name');
  let swapped = false;
  try {
    profiles._internals.open('other', sealed);
  } catch {
    swapped = true;
  }
  assert(swapped, "it cannot be opened as another tenant's profile");

  console.log('\n7️⃣  A dropped client resumes the same session...');
  const r1 = await client();
  const liveId = [...sessions.values()].filter((s) => s.client).slice(-1)[0].id;
  await r1.conn.send('Page.navigate', { url: siteUrl }, r1.sessionId);
  await wait(500);
  r1.conn.close();
  await wait(300);
  assert(sessions.has(liveId), 'the session survives the client going away');
  assert(sessions.get(liveId).client === null, 'it is held with no client attached');

  const resumed = await new CDPConnection(gwUrl(`&session=${liveId}`)).connect();
  const targets = await resumed.send('Target.getTargets');
  assert(targets.targetInfos.length > 0, 'a reconnect resumes against the same browser');
  assert(sessions.get(liveId).client !== null, 'the session shows a client again');
  resumed.close();
  await wait(200);

  console.log('\n8️⃣  Session recording...');
  const rec = await client('&record=1');
  const recId = [...sessions.values()].filter((s) => s.client).slice(-1)[0].id;
  assert(recorder.isRecording(recId), 'recording started for the session');
  await rec.conn.send('Page.navigate', { url: siteUrl }, rec.sessionId);
  await wait(1500);
  await evaluate(rec, 'window.scrollTo(0, 500)');
  await wait(1200);
  await sessions.get(recId).destroy('test done');
  await wait(400);

  const man = await recorder.manifest(recId);
  assert(man && man.frameCount > 0, `frames were captured (${man?.frameCount ?? 0})`);
  const frame0 = await recorder.frame(recId, 0);
  assert(frame0 && frame0[0] === 0xff && frame0[1] === 0xd8, 'stored frames are real JPEGs');
  assert(
    man.frames[0].t >= 0 && man.frames.every((f, i) => i === 0 || f.t >= man.frames[i - 1].t),
    'frame timestamps are monotonic, so a player can scrub',
  );

  console.log('\n9️⃣  Tenant isolation...');
  // Profile names are chosen by callers and are not secrets. Naming another
  // tenant's profile must not hand over their session.
  await wipeBrowser();
  const other = `ws://${origin}/connect?token=admin-key&profile=acme`;
  const thief = await new CDPConnection(other).connect();
  const tTargets = await thief.send('Target.getTargets');
  let tPage = tTargets.targetInfos.find((t) => t.type === 'page');
  const { sessionId: tSid } = await thief.send('Target.attachToTarget', { targetId: tPage.targetId, flatten: true });
  await thief.send('Page.enable', {}, tSid).catch(() => {});
  await thief.send('Page.navigate', { url: siteUrl }, tSid);
  await wait(700);
  const stolen = (await thief.send('Runtime.evaluate', { expression: 'document.cookie', returnByValue: true }, tSid))
    .result?.value;
  assert(!/profile-value/.test(stolen || ''), `another key naming the same profile gets nothing (saw "${stolen}")`);
  const thiefId = [...sessions.values()].filter((s) => s.client).slice(-1)[0].id;
  thief.close();
  await wait(200);
  await sessions.get(thiefId)?.destroy('isolation test');

  // Both keys now have a profile called "acme"; they must be separate stores.
  const fp = (k) => createHash('sha256').update(k).digest('hex').slice(0, 16);
  assert(
    (await profiles.list(fp('tenant-key'))).some((p) => p.name === 'acme'),
    'the owner still lists its own profile',
  );

  await wipeBrowser();
  const back = await client('&profile=acme');
  await back.conn.send('Page.navigate', { url: siteUrl }, back.sessionId);
  await wait(700);
  assert(
    /sid=profile-value/.test((await evaluate(back, 'document.cookie')) || ''),
    "the owner's profile is intact after another key used the same name",
  );
  const backId = [...sessions.values()].filter((s) => s.client).slice(-1)[0].id;
  back.conn.close();
  await wait(200);
  await sessions.get(backId)?.destroy('isolation done');

  console.log('\n🔟  Providers belong to the key that registered them...');
  const fpOf = (k) => createHash('sha256').update(k).digest('hex').slice(0, 16);
  pool.register({
    name: 'mine',
    type: 'cdp',
    wsUrl: chromeWs,
    owner: fpOf('tenant-key'),
    maxConcurrent: 2,
    priority: 5,
  });

  const tenantSees = pool.list(fpOf('tenant-key')).map((p) => p.name);
  const otherSees = pool.list(fpOf('admin-key')).map((p) => p.name);
  assert(tenantSees.includes('mine'), 'the registering key sees its own provider');
  assert(!otherSees.includes('mine'), 'another key does not see it');
  assert(
    tenantSees.includes('local-chrome') && otherSees.includes('local-chrome'),
    'both still see the shared host provider',
  );

  // Two keys can use the same provider name without colliding.
  pool.register({ name: 'mine', type: 'cdp', wsUrl: 'ws://127.0.0.1:1/x', owner: fpOf('admin-key'), maxConcurrent: 1 });
  assert(pool.get(fpOf('tenant-key'), 'mine').wsUrl === chromeWs, 'same name, different owner, different provider');
  assert(pool.get(fpOf('admin-key'), 'mine').wsUrl !== chromeWs, 'the other key has its own');

  pool.remove(fpOf('tenant-key'), 'mine');
  pool.remove(fpOf('admin-key'), 'mine');
  assert(pool.get(fpOf('tenant-key'), 'mine') === undefined, 'removing one does not touch the other owner');

  console.log('\n1️⃣1️⃣  Routing and capacity...');
  const before = pool.stats();
  assert(before.capacity === 4, 'pool reports configured capacity');
  pool.register({ name: 'broken', type: 'cdp', wsUrl: 'ws://127.0.0.1:1/nope', priority: 0, maxConcurrent: 5 });
  const c2 = await client(); // priority 0 is tried first, fails, fails over
  assert(c2.conn, 'a dead provider is failed over rather than failing the client');
  assert(pool.get(null, 'broken').healthy === false, 'the dead provider is put in cooldown');
  assert(pool.get(null, 'local-chrome').active >= 1, 'the session landed on the healthy provider');
  c2.conn.close();
  c1.conn.close();

  console.log('\n9\ufe0f\u20e3  Attach to a browser already in the fleet (?browser=<id>)...');
  {
    const { registry } = await import('../../src/modules/browsers/registry.ts');
    const { CDPDriver } = await import('../../src/drivers/cdp.ts');
    // A fleet browser, as POST /browsers/start would register it.
    const driver = await new CDPDriver({ wsUrl: chromeWs, provider: 'cdp' }).connect();
    registry.add('fleet-1', { apiKey: 'tenant-key', name: 'Fleet 1', clientType: 'cdp', provider: 'cdp', driver });
    await driver.send('navigate', { url: siteUrl + '?fleet=one' });

    const attached = await client('&browser=fleet-1');
    const where = await evaluate(attached, 'location.search');
    assert(where === '?fleet=one', `a Playwright-style client lands on that exact browser (at "${where}")`);
    assert(
      sessions.size >= 1 && [...sessions.values()].some((x) => x.attachedTo === 'fleet-1'),
      'the session records what it is attached to',
    );
    attached.conn.close();
    await new Promise((r) => setTimeout(r, 200));
    assert(registry.isConnected('fleet-1'), 'closing the client does not stop the fleet browser');

    const foreign = new WebSocket(`ws://${origin}/connect?token=admin-key&browser=fleet-1`);
    const foreignResult = await new Promise((resolve) => {
      foreign.once('unexpected-response', (_req, res) => resolve(res.statusCode));
      foreign.once('error', () => resolve('error'));
      foreign.once('open', () => resolve('open'));
    });
    assert(foreignResult === 404, `another key cannot attach to it (got ${foreignResult})`);

    registry.add('oya-1', {
      apiKey: 'tenant-key',
      name: 'Oya 1',
      clientType: 'oya',
      provider: 'oya-desktop',
      ws: { close() {}, send() {} },
    });
    const notCdp = new WebSocket(`ws://${origin}/connect?token=tenant-key&browser=oya-1`);
    const notCdpResult = await new Promise((resolve) => {
      notCdp.once('unexpected-response', (_req, res) => resolve(res.statusCode));
      notCdp.once('error', () => resolve('error'));
      notCdp.once('open', () => resolve('open'));
    });
    assert(
      notCdpResult === 409,
      `an Oya client with its front door off has nothing to attach to (got ${notCdpResult})`,
    );

    // An Oya client with its front door on: CDP rides its control socket. This
    // stands in for main.js's relay, bridging to the real Chrome.
    const { onBrowserMessage } = await import('../../src/modules/browsers/cdp-relay.ts');
    const local = new Map(),
      told = [];
    const control = {
      close() {},
      send(text) {
        const m = JSON.parse(text);
        told.push(m.type);
        if (m.type === 'cdp_open') {
          const sock = new WebSocket(chromeWs);
          local.set(m.sid, sock);
          sock.on('open', () => onBrowserMessage('oya-2', { type: 'cdp_opened', sid: m.sid }));
          sock.on('message', (d) => onBrowserMessage('oya-2', { type: 'cdp', sid: m.sid, data: d.toString() }));
        }
        if (m.type === 'cdp') local.get(m.sid).send(m.data);
        if (m.type === 'cdp_close') local.get(m.sid).close();
      },
    };
    registry.add('oya-2', {
      apiKey: 'tenant-key',
      name: 'Oya 2',
      clientType: 'oya',
      provider: 'oya-cloud',
      cdp: true,
      ws: control,
    });
    const relayed = await client('&browser=oya-2');
    const relayedWhere = await evaluate(relayed, 'location.search');
    assert(relayedWhere === '?fleet=one', `a CDP client drives an Oya client through the relay (at "${relayedWhere}")`);
    relayed.conn.close();
    // The session outlives its client for the grace period, so end it the way expiry would.
    await [...sessions.values()].find((x) => x.attachedTo === 'oya-2').destroy('grace expired');
    assert(told.includes('cdp_close'), 'ending the session closes the relay in the browser');
    assert(registry.isConnected('oya-2'), 'and the Oya browser stays in the fleet');
    registry.remove('fleet-1');
    registry.remove('oya-1');
    registry.remove('oya-2');
  }
} catch (e) {
  console.log(`  ❌ threw: ${e.message}\n${e.stack?.split('\n').slice(0, 4).join('\n')}`);
  failed++;
} finally {
  for (const s of [...sessions.values()]) await s.destroy('teardown').catch(() => {});
  chrome.kill('SIGKILL');
  await new Promise((r) => server.close(r));
  await new Promise((r) => site.close(r));
  removeScratch(DATA);
}

console.log('\n──────────────────────────────────────────────────');
console.log(`  ${passed} passed, ${failed} failed`);
console.log('──────────────────────────────────────────────────');
process.exit(failed ? 1 : 0);
