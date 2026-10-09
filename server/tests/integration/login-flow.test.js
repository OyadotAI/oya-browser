/** Native SDK login persistence: cold hydration, correlated final capture, encrypted durability and fresh-engine logout. */
import assert from 'node:assert/strict';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { once } from 'node:events';
import { createServer } from 'node:http';
import { openNativeFixture } from '../support/native-browser.mjs';
import { removeScratch } from '../support/scratch.js';
const scratch = mkdtempSync(join(tmpdir(), 'oya-native-profile-sync-'));
process.env.OYA_DATA_DIR = scratch;
process.env.API_KEYS = 'native-profile-key,other-key';
process.env.OYA_ALLOW_PRIVATE_TARGETS = 'true';
process.env.OYA_PROFILE_SECRET = 'native-profile-test-secret';
const [
  { default: express },
  { WebSocketServer, WebSocket },
  { router },
  { handleConnection },
  { registry },
  { container },
  logins,
  { Oya },
] = await Promise.all([
  import('express'),
  import('ws'),
  import('../../src/app/api.ts'),
  import('../../src/modules/browsers/socket.ts'),
  import('../../src/modules/browsers/registry.ts'),
  import('../../src/app/container.ts'),
  import('../../src/modules/personas/cookies.ts'),
  import('../../../packages/sdk/dist/index.js'),
]);
const app = express();
app.use(express.json());
app.use('/api', router);
app.get('/fixture', (req, res) =>
  res.type('html').send(`<!doctype html><script>
document.title=${JSON.stringify((req.headers.cookie || '').includes('account=alice'))} && localStorage.getItem('account')==='alice' ? 'Signed in as Alice' : 'Signed out';
</script><h1>Native profile fixture</h1><label>Name <input value="old" placeholder="Name"></label><button onclick="document.title='Clicked';window.trusted=event.isTrusted">Press me</button>`),
);
const gateway = await import('../../src/modules/gateway/service.ts');
const server = createServer(app),
  wss = new WebSocketServer({ noServer: true });
wss.on('connection', handleConnection);
server.on('upgrade', (req, socket, head) => {
  if (req.url.startsWith('/connect')) return gateway.handleUpgrade(req, socket, head);
  wss.handleUpgrade(req, socket, head, (ws) => wss.emit('connection', ws, req));
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`,
  fixtures = [],
  sockets = [];
const relays = new Map();
let captureCount = 0,
  refuseCapture = false;
/** The private bridge carries native operations; the real authenticated socket carries ordered profile updates. */
async function connect(id) {
  const fixture = await openNativeFixture();
  fixtures.push(fixture);
  const socket = new WebSocket(base.replace('http:', 'ws:') + '/ws');
  sockets.push(socket);
  let endpoint;
  const ready = new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(Error('Native profile authentication timed out')), 15000);
    socket.on('message', async (raw) => {
      try {
        const msg = JSON.parse(raw);
        if (msg.type === 'auth_ok') {
          await fixture.prepareProfile(msg);
          endpoint = await fixture.frontDoor();
          clearTimeout(timeout);
          resolve();
        }
        if (msg.type.startsWith('cdp')) await relay(socket, msg, endpoint);
        if (msg.type === 'ping') socket.send(JSON.stringify({ type: 'pong' }));
        if (msg.type === 'profile_capture') {
          captureCount++;
          const replies = refuseCapture
            ? [{ type: 'cmd_result', id: msg.id, ok: false }]
            : await fixture.captureProfile(msg.id);
          for (const reply of replies) socket.send(JSON.stringify(reply));
        }
        if (msg.type === 'cmd')
          socket.send(
            JSON.stringify({ ...(await fixture.send(msg.action, msg.params)), type: 'cmd_result', id: msg.id }),
          );
      } catch (error) {
        clearTimeout(timeout);
        reject(error);
        socket.close();
      }
    });
    socket.once('error', (error) => {
      clearTimeout(timeout);
      reject(error);
    });
    socket.once('close', () => {
      clearTimeout(timeout);
      reject(Error('Native profile socket closed'));
    });
  });
  await once(socket, 'open');
  socket.send(
    JSON.stringify({
      type: 'auth',
      api_key: 'native-profile-key',
      browser_id: id,
      browser_name: id,
      provider: 'oya-desktop',
      cdp: true,
      profile_sync: true,
      actions: ['navigate', 'evaluate_raw', 'analyze', 'click', 'type', 'list_tabs'],
    }),
  );
  await ready;
  return fixture;
}
/** Carry only external agent protocol frames to Oya's native adapter, not to an engine debugging endpoint. */
async function relay(socket, msg, endpoint) {
  if (msg.type === 'cdp_open') {
    const peer = new WebSocket(endpoint.url, { headers: { authorization: `Bearer ${endpoint.token}` } });
    relays.set(msg.sid, peer);
    peer.on('message', (data) => socket.send(JSON.stringify({ type: 'cdp', sid: msg.sid, data: data.toString() })));
    peer.on('close', () => {
      relays.delete(msg.sid);
      if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify({ type: 'cdp_closed', sid: msg.sid }));
    });
    await once(peer, 'open');
    socket.send(JSON.stringify({ type: 'cdp_opened', sid: msg.sid }));
  }
  if (msg.type === 'cdp') relays.get(msg.sid)?.send(msg.data);
  if (msg.type === 'cdp_close') relays.get(msg.sid)?.close();
}
/** External front-door discovery through the returned public gateway URL. */
async function targets(url) {
  const client = new WebSocket(url);
  try {
    await once(client, 'open');
    const answer = new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(Error('Native target discovery timed out')), 10000);
      client.on('message', (raw) => {
        const value = JSON.parse(raw);
        if (value.id === 1) {
          clearTimeout(timeout);
          value.error ? reject(Error(value.error.message)) : resolve(value.result);
        }
      });
    });
    client.send(JSON.stringify({ id: 1, method: 'Target.getTargets', params: {} }));
    return await answer;
  } finally {
    client.terminate();
  }
}

try {
  const profile = container.personas.defaultFor('native-profile-key');
  logins.mergeDump(profile.id, [
    { name: 'account', value: 'alice', domain: '127.0.0.1', path: '/', httpOnly: true, hostOnly: true },
  ]);
  logins.mergeStorage(profile.id, { [base]: { account: 'alice' } });
  await logins.drain();
  const reloaded = await import(`../../src/modules/personas/cookies.ts?native=${Date.now()}`);
  await reloaded.restore();
  assert.equal(reloaded.getAll(profile.id)[0].value, 'alice');
  assert.equal(reloaded.getStorage(profile.id)[base].account, 'alice');
  assert.equal(reloaded.getAll(container.personas.defaultFor('other-key').id).length, 0);
  const mfa = await import('../../src/modules/challenges/mfa.ts');
  await mfa.set(profile.id, { type: 'totp', secret: 'JBSWY3DPEHPK3PXP' });
  const storage = await import('../../src/platform/storage/index.ts');
  assert.ok(
    !(await storage.getConnection().select('mfa_factors'))
      .map((row) => row.value)
      .join('')
      .includes('JBSW'),
  );
  mfa.reset();
  await mfa.restore();
  assert.equal(mfa.describe(profile.id).type, 'totp');
  console.log('PASS: encrypted login and MFA state survive module reload and remain tenant-scoped');
  const oya = new Oya({ apiKey: 'native-profile-key', baseUrl: base });
  const first = await connect('native-profile-first');
  const browser = await oya.browser.start({ name: 'native-profile-first', profile: 'default' });
  await browser.goto(base + '/fixture');
  assert.equal(await first.evaluateMain('document.title'), 'Signed in as Alice');
  assert.equal((await first.cookies()).find((c) => c.name === 'account').httpOnly, true);
  assert.equal((await browser.tabs())[0].title, 'Signed in as Alice');
  const page = await browser.analyze();
  await browser.type(page.elements.find((el) => el.tag === 'input').id, 'replacement');
  assert.equal(await first.evaluateMain("document.querySelector('input').value"), 'replacement');
  await browser.click(page.elements.find((el) => el.text === 'Press me').id);
  assert.equal(await first.evaluateMain('window.trusted'), true);
  assert.equal((await browser.tabs())[0].title, 'Clicked');
  const url = new URL(browser.cdpUrl);
  assert.equal(url.searchParams.get('browser'), browser.id);
  const refreshed = new URL((await oya.browser.get(browser.id)).cdpUrl);
  assert.equal(refreshed.searchParams.get('browser'), browser.id);
  assert.ok(refreshed.searchParams.has('ticket'));
  assert.equal(refreshed.searchParams.has('token'), false);
  assert.ok((await targets(browser.cdpUrl)).targetInfos.some((target) => target.title === 'Clicked'));
  console.log('PASS: SDK first-script native hydration, trusted typing/clicking, and returned native CDP front door');
  refuseCapture = true;
  assert.equal((await browser.stop()).reused, true, 'borrowed desktop handles never close the person’s browser');
  const refused = await oya.browser.stop([browser.id]);
  assert.equal(refused.results[0].ok, false);
  assert.match(refused.results[0].error, /save profile|capture failed/i);
  assert.ok(registry.get(browser.id), 'failed capture must not stop the browser');
  refuseCapture = false;
  await first.evaluateMain('localStorage.clear(); document.cookie="newSession=renewed; path=/"');
  await browser.goto(base + '/fixture');
  assert.equal(await first.evaluateMain('document.title'), 'Signed out');
  assert.equal((await oya.browser.stop([browser.id])).stopped, 1);
  assert.equal(captureCount, 2);
  assert.deepEqual(logins.getStorage(profile.id)[base], {});
  assert.ok(logins.getAll(profile.id).some((c) => c.name === 'newSession' && c.value === 'renewed'));
  const { getConnection } = await import('../../src/platform/storage/index.ts');
  const stored = (await getConnection().select('persona_logins')).map((row) => row.value).join('\n');
  assert.ok(!stored.includes('renewed') && !stored.includes('alice'), 'durable profile values remain encrypted');
  assert.deepEqual(logins.getStorage(container.personas.defaultFor('other-key').id), {});
  console.log(
    'PASS: stop refuses failed capture, then captures logout and refreshed cookies before encrypted persistence',
  );
  const second = await connect('native-profile-second');
  const next = await oya.browser.start({ name: 'native-profile-second' });
  await next.goto(base + '/fixture');
  assert.equal(await second.evaluateMain('document.title'), 'Signed out');
  assert.ok((await second.cookies()).some((c) => c.name === 'newSession'));
  assert.equal((await oya.browser.stop([next.id])).stopped, 1);
  assert.equal(captureCount, 3);
  assert.equal(await oya.browser.stopAll(), 0);
  console.log('PASS: a fresh native engine restores refreshed cookies without resurrecting cleared localStorage');
} finally {
  for (const peer of relays.values()) peer.terminate();
  for (const session of gateway.sessions.values()) await session.destroy('test complete');
  for (const socket of sockets) socket.terminate();
  for (const fixture of fixtures) await fixture.close();
  for (const [id] of registry.browsers) registry.remove(id);
  await logins.drain();
  wss.close();
  server.closeAllConnections();
  await new Promise((resolve) => server.close(resolve));
  removeScratch(scratch);
}
process.exit(0);
