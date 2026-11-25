/**
 * Real Electron check of the commands the agent's page tools send: history moves
 * that wait for the page they land on, a read-only script in the analyzer's
 * isolated world (which the page cannot see into, nor the script into the page's
 * globals), hovering an element, reading what a tab logged and fetched without
 * seeing another tab's, and a wait for text that ends on time. A fake control
 * server sends each command as the Oya server would and reads its cmd_result.
 * Run: npm run test:agent.
 */
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { createServer } from 'node:http';
import { WebSocketServer } from 'ws';
import { _electron as electron } from '../../../ui/node_modules/playwright/index.mjs';
/** The persona a real server sends in auth_ok; the desktop refuses sign-in without one. */
const PERSONA = createRequire(import.meta.url)('../../anonymity/fingerprint.js').generateProfile({
  seed: 'agent-cmds',
  platform: 'MacIntel',
});

/** What two pages log and fetch, so a read of one can be shown to hold nothing of the other. */
const NOISE = {
  '/noisy': `<script>console.error('boom-in-my-tab'); fetch('/refused')</script><p>Loaded</p>`,
  '/other': `<script>console.error('marker-in-their-tab'); fetch('/also-refused')</script>`,
};
/** The status a path answers with: the two refused paths are what a failed request looks like. */
const NOT_FOUND = 404;
const statusFor = (path) => (path.endsWith('refused') ? NOT_FOUND : undefined);

/** A page per path: its title names it, a global the isolated world must not see, and a hover target. */
const pageFor = (path) =>
  `<!doctype html><title>page ${path}</title><script>window.pageSecret = 'x'</script>${NOISE[path] || ''}` +
  `<button id="menu" onmouseenter="document.title='hovered'">Menu</button><table><tr><td>1</td></tr><tr><td>2</td></tr></table>`;

/** How long the wait that must fail waits, and the most it may take to say so. */
const WAIT_MS = 500;
const WAIT_SLACK_MS = 5000;

const profile = await mkdtemp(join(tmpdir(), 'oya-agent-commands-'));
const fixture = createServer((req, res) => res.writeHead(statusFor(req.url) ?? 200).end(pageFor(req.url)));
await new Promise((resolve) => fixture.listen(0, '127.0.0.1', resolve));
const origin = `http://127.0.0.1:${fixture.address().port}`;
const wss = new WebSocketServer({ port: 0, host: '127.0.0.1' });
await new Promise((resolve) => wss.once('listening', resolve));
const waiting = new Map();
let socket,
  app,
  next = 0;
wss.on('connection', (ws) => {
  socket = ws;
  ws.on('message', (raw) => {
    const message = JSON.parse(raw);
    const control = { mode: 'agent', mine: false, revision: 1 };
    if (message.type === 'auth')
      ws.send(JSON.stringify({ type: 'auth_ok', browser_id: 'agent-cmds', control, fingerprint: PERSONA }));
    if (message.type === 'ping') ws.send(JSON.stringify({ type: 'pong' }));
    if (message.type === 'cmd_result') waiting.get(message.id)?.(message);
  });
});

/** Sends one command as the server would and answers its result. */
function command(action, params = {}) {
  const id = `c${next++}`;
  const answered = new Promise((resolve) => waiting.set(id, resolve));
  socket.send(JSON.stringify({ type: 'cmd', id, action, params }));
  return answered;
}

/** The value a read-only script returned, failing on anything else. */
async function read(script) {
  const r = await command('run_script', { script });
  assert.equal(r.ok, true, r.error);
  assert.equal(r.data.error, undefined, r.data.error);
  return r.data.value;
}

try {
  app = await electron.launch({
    executablePath: createRequire(import.meta.url)('electron'),
    args: [fileURLToPath(new URL('../../', import.meta.url))],
    cwd: fileURLToPath(new URL('../../', import.meta.url)),
    env: {
      ...process.env,
      OYA_USER_DATA_DIR: profile,
      // Connects at launch with this key: the setup screens are not what this suite checks, and
      // clicking through them broke it each time they were redesigned.
      OYA_API_KEY: 'test',
      OYA_AUTO_CONNECT: 'true',
      OYA_SERVER_URL: `ws://127.0.0.1:${wss.address().port}`,
      OYA_REMOTE_DEBUGGING_PORT: '0',
    },
  });
  const page = await app.firstWindow();
  page.setDefaultTimeout(10000);
  // Connected with no agent at work: the person is using the browser, so Human Mode shows.
  await page.locator('#control-action[aria-pressed="true"]').waitFor();

  assert.equal((await command('navigate', { url: `${origin}/a` })).ok, true);
  assert.equal((await command('navigate', { url: `${origin}/b` })).ok, true);
  assert.equal(await read('return document.title'), 'page /b');

  assert.equal((await command('back')).ok, true);
  assert.equal(await read('return location.pathname'), '/a', 'back lands on the previous page');
  assert.equal((await command('forward')).ok, true);
  assert.equal(await read('return location.pathname'), '/b', 'forward lands on the next page');
  assert.equal((await command('reload')).ok, true);
  assert.equal(await read('return document.title'), 'page /b', 'reload waits for the page');

  assert.deepEqual(await read("return [...document.querySelectorAll('td')].map((td) => td.textContent)"), ['1', '2']);
  assert.equal(await read('return typeof window.pageSecret'), 'undefined', 'the script runs apart from the page');
  assert.equal(await read('await new Promise((r) => setTimeout(r, 10)); return 7'), 7, 'the script may await');
  const loads = "return performance.getEntriesByType('navigation').length";
  assert.equal(await read(loads), 1, "wait_for's network reading works apart from the page");
  const threw = await command('run_script', { script: 'return missing.value' });
  assert.match(threw.data.error, /missing is not defined/);

  const analysis = await command('analyze');
  const menu = analysis.data.elements.find((e) => e.text === 'Menu');
  const hovered = await command('hover', { selector: `[data-ac-id="${menu.id}"]` });
  assert.equal(hovered.ok, true, hovered.error);
  assert.equal(await read('return document.title'), 'hovered', 'hover reached the element');

  // A tab's console and requests are read from that tab alone: the rings hold every tab's.
  assert.equal((await command('navigate', { url: `${origin}/other` })).ok, true);
  const theirs = (await command('list_tabs')).data.tabs.find((t) => t.url.endsWith('/other')).id;
  assert.equal((await command('open_tab', { url: `${origin}/noisy` })).ok, true);
  assert.equal((await command('wait', { text: 'Loaded' })).ok, true, 'a wait for text the page shows succeeds');
  const logged = (await command('read_console')).data.entries.map((e) => e.message);
  assert.ok(logged.includes('boom-in-my-tab'), `my tab's error is read: ${logged}`);
  assert.ok(!logged.includes('marker-in-their-tab'), "another tab's console is never read");
  const refused = (await command('read_network', { failedOnly: true })).data.requests;
  assert.deepEqual(
    refused.map((r) => [new URL(r.url).pathname, r.status]),
    [['/refused', NOT_FOUND]],
    "only my tab's failed request is read",
  );
  // A caller cannot name another page to read.
  const named = (await command('read_console', { webContentsId: 1 })).data.entries.map((e) => e.message);
  assert.ok(!named.includes('marker-in-their-tab'), 'a page id in the request is ignored');
  // The other tab's entries exist and are read once that tab is the lane's.
  assert.equal((await command('switch_tab', { tab_id: theirs })).ok, true);
  const afterSwitch = (await command('read_console')).data.entries.map((e) => e.message);
  assert.ok(afterSwitch.includes('marker-in-their-tab') && !afterSwitch.includes('boom-in-my-tab'));

  const started = Date.now();
  const never = await command('wait', { text: 'never shown', timeout: WAIT_MS });
  assert.equal(never.ok, false);
  assert.match(never.error, /Timed out after 500 ms: "never shown" did not show/);
  assert.ok(Date.now() - started < WAIT_SLACK_MS, 'a wait that runs out ends on time');
  const nothing = await command('wait', {});
  assert.match(nothing.error, /Nothing to wait for/, 'a wait for nothing fails at once');
  console.log(
    'Agent commands passed: back, forward, reload, run_script in the isolated world, hover, per-tab console and network, wait for text.',
  );
} finally {
  await app?.close();
  for (const ws of wss.clients) ws.terminate();
  await new Promise((resolve) => wss.close(resolve));
  await new Promise((resolve) => fixture.close(resolve));
  await rm(profile, { recursive: true, force: true });
}
