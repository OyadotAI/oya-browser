/**
 * Real Electron check of the commands the agent's page tools send: history moves
 * that wait for the page they land on, a read-only script in the analyzer's
 * isolated world (which the page cannot see into, nor the script into the page's
 * globals), and hovering an element. A fake control server sends each command as
 * the Oya server would and reads its cmd_result.
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

/** A page per path: its title names it, a global the isolated world must not see, and a hover target. */
const pageFor = (path) =>
  `<!doctype html><title>page ${path}</title><script>window.pageSecret = 'x'</script>` +
  `<button id="menu" onmouseenter="document.title='hovered'">Menu</button><table><tr><td>1</td></tr><tr><td>2</td></tr></table>`;

const profile = await mkdtemp(join(tmpdir(), 'oya-agent-commands-'));
const fixture = createServer((req, res) => res.end(pageFor(req.url)));
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
    if (message.type === 'auth') ws.send(JSON.stringify({ type: 'auth_ok', browser_id: 'agent-cmds', control }));
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
    args: [fileURLToPath(new URL('../../main.js', import.meta.url))],
    cwd: fileURLToPath(new URL('../../', import.meta.url)),
    env: {
      ...process.env,
      OYA_USER_DATA_DIR: profile,
      OYA_API_KEY: '',
      OYA_AUTO_CONNECT: 'false',
      OYA_SERVER_URL: `ws://127.0.0.1:${wss.address().port}`,
      OYA_REMOTE_DEBUGGING_PORT: '0',
    },
  });
  const page = await app.firstWindow();
  page.setDefaultTimeout(10000);
  await page.locator('#btn-manual').click();
  await page.locator('#cfg-key').fill('test');
  await page.locator('#btn-connect').click();
  await page.getByText('Agent control', { exact: true }).waitFor();

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
  console.log('Agent commands passed: back, forward, reload, run_script in the isolated world, hover.');
} finally {
  await app?.close();
  for (const ws of wss.clients) ws.terminate();
  await new Promise((resolve) => wss.close(resolve));
  await new Promise((resolve) => fixture.close(resolve));
  await rm(profile, { recursive: true, force: true });
}
