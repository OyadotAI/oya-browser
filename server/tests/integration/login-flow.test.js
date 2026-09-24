#!/usr/bin/env node
/** The public SDK journey, against real Chrome and the real control plane. */
import assert from 'node:assert/strict';
import { mkdtempSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createServer } from 'node:http';
import { removeScratch } from '../support/scratch.js';

const chromePath = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
].find(existsSync);
assert(chromePath, 'Install Chrome to run the login journey test');
const scratch = mkdtempSync(join(tmpdir(), 'oya-login-'));
process.env.OYA_DATA_DIR = scratch;
process.env.API_KEYS = 'login-test,other-test';
process.env.OYA_ALLOW_PRIVATE_TARGETS = 'true';
process.env.OYA_PROFILE_SECRET = 'local-login-test-secret';
const [
  { default: express },
  { WebSocketServer },
  { router },
  { handleConnection },
  gateway,
  { registry },
  profiles,
  logins,
  mfa,
  { Oya },
] = await Promise.all([
  import('express'),
  import('ws'),
  import('../../src/app/api.ts'),
  import('../../src/modules/browsers/socket.ts'),
  import('../../src/modules/gateway/service.ts'),
  import('../../src/modules/browsers/registry.ts'),
  import('../../src/app/container.ts').then((m) => m.container.personas),
  import('../../src/modules/personas/cookies.ts'),
  import('../../src/modules/challenges/mfa.ts'),
  import('../../../packages/sdk/dist/index.js'),
]);
const app = express();
app.use(express.json());
app.use('/api', router);
app.get('/fixture', (req, res) =>
  res.type('html').send(`<!doctype html><title>loading</title>
<script>document.title = ${JSON.stringify((req.headers.cookie || '').includes('account=alice'))} && localStorage.getItem('account') === 'alice' ? 'Signed in as Alice' : 'Signed out';</script>
<label>Name <input value="old" placeholder="Name"></label><button onclick="document.title='Clicked'">Press me</button>`),
);
const server = createServer(app);
const wss = new WebSocketServer({ noServer: true });
wss.on('connection', handleConnection);
server.on('upgrade', (req, socket, head) => {
  if (req.url.startsWith('/connect')) return gateway.handleUpgrade(req, socket, head);
  wss.handleUpgrade(req, socket, head, (ws) => wss.emit('connection', ws, req));
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const base = `http://127.0.0.1:${server.address().port}`;
const children = [];
async function chrome() {
  const child = spawn(
    chromePath,
    [
      '--headless=new',
      '--remote-debugging-port=0',
      '--no-first-run',
      '--no-default-browser-check',
      `--user-data-dir=${join(scratch, `chrome-${children.length}`)}`,
      'about:blank',
    ],
    { stdio: ['ignore', 'ignore', 'pipe'] },
  );
  children.push(child);
  return new Promise((resolve, reject) => {
    let output = '';
    const timer = setTimeout(() => reject(new Error('Chrome did not start')), 20_000);
    child.stderr.on('data', (data) => {
      output += data;
      const url = output.match(/ws:\/\/127\.0\.0\.1:\d+\/devtools\/browser\/[a-f0-9-]+/);
      if (url) {
        clearTimeout(timer);
        resolve(url[0]);
      }
    });
    child.once('error', reject);
  });
}
const passed = (name) => console.log(`PASS ${name}`);
try {
  const profile = profiles.defaultFor('login-test');
  logins.mergeDump(profile.id, [
    { name: 'account', value: 'alice', domain: '127.0.0.1', path: '/', httpOnly: true, hostOnly: true },
  ]);
  logins.mergeStorage(profile.id, { [base]: { account: 'alice' } });
  await logins.drain();
  const { getConnection } = await import('../../src/platform/storage/index.ts');
  /** Every stored value of a record table, joined, to search for plaintext. */
  const stored = async (table) => (await getConnection().select(table)).map((r) => r.value).join('\n');
  assert(!(await stored('persona_logins')).includes('alice'));
  const reloaded = await import(`../../src/modules/personas/cookies.ts?reload=${Date.now()}`);
  await reloaded.restore();
  assert.equal(reloaded.getAll(profile.id)[0].value, 'alice');
  assert.equal(reloaded.getStorage(profile.id)[base].account, 'alice');
  assert.equal(reloaded.getAll(profiles.defaultFor('other-test').id).length, 0);
  passed('encrypted profile survives a fresh module load and stays tenant scoped');

  await mfa.set(profile.id, { type: 'totp', secret: 'JBSWY3DPEHPK3PXP' });
  assert(!(await stored('mfa_factors')).includes('JBSW'));
  mfa.reset();
  await mfa.restore();
  assert.equal(mfa.describe(profile.id).type, 'totp');
  passed('MFA configuration survives restart without plaintext secrets');

  const oya = new Oya({ apiKey: 'login-test', baseUrl: base });
  await oya.config.set({ browser_provider: 'cdp', cdp_ws_url: await chrome() });
  const browser = await oya.browser.start({ profile: 'default', captcha: 'auto' });
  await browser.goto(`${base}/fixture`);
  assert.equal((await browser.tabs())[0].title, 'Signed in as Alice');
  assert.equal(new URL(browser.cdpUrl).searchParams.get('browser'), browser.id);
  const refreshedUrl = new URL((await oya.browser.get(browser.id)).cdpUrl);
  assert.equal(refreshedUrl.searchParams.get('browser'), browser.id);
  assert.ok(refreshedUrl.searchParams.get('ticket'));
  assert.equal(refreshedUrl.searchParams.has('token'), false);
  passed('six-line SDK path restores cookies and localStorage before the first page script');

  const page = await browser.analyze();
  const input = page.elements.find((el) => el.tag === 'input');
  const button = page.elements.find((el) => el.text === 'Press me');
  await browser.type(input.id, 'replacement');
  assert.equal(
    await registry.get(browser.id).driver.engine.evaluateMain("document.querySelector('input').value"),
    'replacement',
  );
  await browser.click(button.id);
  assert.equal((await browser.tabs())[0].title, 'Clicked');
  passed('SDK element IDs click and replace input values on CDP');

  const { CDPConnection } = await import('../../src/drivers/cdp.ts');
  const attached = await new CDPConnection(browser.cdpUrl).connect();
  const targets = await attached.send('Target.getTargets');
  assert(targets.targetInfos.some((t) => t.title === 'Clicked'));
  attached.close();
  passed('returned CDP endpoint attaches to this browser through the gateway');

  await registry
    .get(browser.id)
    .driver.engine.evaluateMain("localStorage.clear(); document.cookie='newSession=renewed; path=/'");
  await new Promise((r) => setTimeout(r, 150));
  await browser.goto(`${base}/fixture`);
  assert.equal(await registry.get(browser.id).driver.engine.evaluateMain("localStorage.getItem('account')"), null);
  await browser.stop();
  assert(logins.getAll(profile.id).some((c) => c.name === 'newSession' && c.value === 'renewed'));
  assert.deepEqual(logins.getStorage(profile.id)[base], {});
  passed('logout stays logged out on navigation, and stop saves refreshed cookies');

  await oya.config.set({ cdp_ws_url: await chrome() });
  const next = await oya.browser.start();
  await next.goto(`${base}/fixture`);
  assert.equal((await next.tabs())[0].title, 'Signed out');
  assert((await registry.get(next.id).driver.cookies()).some((c) => c.name === 'newSession'));
  await next.stop();
  assert.equal(await oya.browser.stopAll(), 0);
  passed('a fresh browser restores updated state; stopping an empty fleet is harmless');
} finally {
  for (const session of gateway.sessions.values()) await session.destroy('test complete');
  for (const [id] of registry.browsers) registry.remove(id);
  for (const child of children)
    if (child.exitCode == null) {
      const done = once(child, 'exit');
      child.kill('SIGTERM');
      await done;
    }
  await logins.drain();
  await new Promise((r) => server.close(r));
  removeScratch(scratch);
}
process.exit(0);
