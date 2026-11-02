/**
 * Real Electron check of tearing a tab off, as Chrome does it, in AI Mode and in
 * Human Mode against a fake control server: the tab is in its own window while
 * the button is still down, that window follows the pointer, and a one-tab
 * window dragged by its tab shows in a gap over another strip, its window
 * hidden, and lands in that gap when let go.
 * Run: npm run test:tear-off.
 */
/* global window, document */
import assert from 'node:assert/strict';
import { mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { createServer } from 'node:http';
import { WebSocketServer } from 'ws';
import { _electron as electron } from '../../../ui/node_modules/playwright/index.mjs';
const PERSONA = createRequire(import.meta.url)('../../anonymity/fingerprint.js').generateProfile({
  seed: 'control-ui',
  platform: 'MacIntel',
});
const profile = await mkdtemp(join(tmpdir(), 'oya-tear-'));
await writeFile(join(profile, 'config.json'), JSON.stringify({ ui: { importOffered: true } }));
const fixture = createServer((req, res) => res.end(`<!doctype html><title>${req.url}</title><p>${req.url}`));
await new Promise((r) => fixture.listen(0, '127.0.0.1', r));
const url = `http://127.0.0.1:${fixture.address().port}/`;
const wss = new WebSocketServer({ port: 0, host: '127.0.0.1' });
await new Promise((r) => wss.once('listening', r));
let state = { mode: 'agent', mine: false, revision: 1 };
wss.on('connection', (ws) =>
  ws.on('message', (raw) => {
    const m = JSON.parse(raw);
    if (m.type === 'auth')
      ws.send(JSON.stringify({ type: 'auth_ok', browser_id: 'x', control: state, fingerprint: PERSONA }));
    if (m.type === 'ping') ws.send(JSON.stringify({ type: 'pong' }));
    if (m.type !== 'desktop_control') return;
    const modes = { request: 'paused', acquire: 'human', return: 'agent', renew: 'human' };
    state = {
      mode: modes[m.action],
      mine: m.action !== 'return',
      expiresAt: Date.now() + 300000,
      revision: state.revision + 1,
    };
    setTimeout(() => ws.send(JSON.stringify({ type: 'desktop_control_result', id: m.id, state })), 60);
  }),
);
const mode = process.argv[2] || 'agent';
const app = await electron.launch({
  executablePath: createRequire(import.meta.url)('electron'),
  args: [fileURLToPath(new URL('../../', import.meta.url))],
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
const shells = () =>
  app.evaluate(({ BrowserWindow }) =>
    BrowserWindow.getAllWindows()
      .filter((w) => w.webContents.getURL().includes('/out/renderer/index.html') && w.isVisible())
      .map((w) => ({ id: w.id, pos: w.getPosition(), b: w.getContentBounds() })),
  );
const cursor = (x, y) =>
  app.evaluate(
    ({ screen }, p) => {
      screen.getCursorScreenPoint = () => p;
    },
    { x, y },
  );
const count = (p) => p.evaluate(() => document.querySelectorAll('#tab-list .tab-item:not(.closing)').length);
/** Drags the tab at `sel` out of page `p`'s strip, moves the cursor along `path`, and lets go at `dropAt` (after `probe` looks); answers the windows during, after the moves, and after release. */
async function drag(p, sel, path, dropAt, probe = async () => {}) {
  const b = await p.locator(sel).boundingBox();
  const x = b.x + b.width / 2,
    y = b.y + b.height / 2;
  await p.mouse.move(x, y);
  await p.mouse.down();
  for (let i = 1; i <= 15; i++) {
    await p.mouse.move(x, y + i * 12);
    await p.waitForTimeout(16);
  }
  await p.waitForTimeout(1500);
  const during = await shells();
  for (const [cx, cy] of path) {
    await cursor(cx, cy);
    await p.waitForTimeout(120);
  }
  const moved = await shells();
  if (dropAt) await cursor(...dropAt);
  await p.waitForTimeout(300);
  await probe();
  await p.mouse.up();
  await new Promise((r) => setTimeout(r, 1500));
  return { during, moved, after: await shells() };
}
try {
  const page = await app.firstWindow();
  page.setDefaultTimeout(8000);
  await page.locator('#tab-key').click();
  await page.locator('#btn-manual').click();
  await page.locator('#cfg-key').fill('test');
  await page.locator('#btn-connect').click();
  await page.locator('#control-agent[aria-pressed="true"]').waitFor();
  await page.waitForFunction(() => document.body.classList.contains('mode-browsing'));
  if (mode === 'human') {
    await app.evaluate(({ dialog }) => {
      dialog.showMessageBox = async () => ({ response: 1, checkboxChecked: false });
    });
    await page.locator('#control-action').click();
    await page.locator('#control-action[aria-pressed="true"]').waitFor();
  }
  for (const n of ['a', 'b']) await page.evaluate((u) => window.oyaBrowser.newTab(u), url + n);
  await page.waitForTimeout(1200);
  const [w1] = await shells();
  // 1. Tear the active tab off: a real window should appear before release and follow the cursor.
  await cursor(w1.b.x + 200, w1.b.y + 200);
  const r1 = await drag(page, '#tab-list .tab-item.active', [
    [w1.b.x + 400, w1.b.y + 400],
    [w1.b.x + 500, w1.b.y + 450],
  ]);
  assert.equal(r1.during.length, 2, 'the torn-off tab has its own window before the button comes up');
  const carried = (r) => r.find((w) => w.id !== w1.id).pos;
  assert.deepEqual(
    carried(r1.moved),
    [carried(r1.during)[0] + 300, carried(r1.during)[1] + 250],
    'the window follows the pointer',
  );
  assert.equal(r1.after.length, 2, 'let go over no strip, it stays a window of its own');
  // 2. Drag the new one-tab window's tab back onto the first window's strip: it should merge.
  const second = app.windows().find((p) => p !== page && p.url().includes('/out/renderer/index.html'));
  const w2 = (await shells()).find((w) => w.id !== w1.id);
  await cursor(w2.b.x + 100, w2.b.y + 100);
  const before = await count(page);
  const moving = Number(await second.locator('#tab-list .tab-item.active').getAttribute('data-id'));
  const first = await page.locator('#tab-list .tab-item').first().boundingBox();
  const hovering = {};
  const r2 = await drag(
    second,
    '#tab-list .tab-item.active',
    [[w2.b.x + 150, w2.b.y + 150]],
    [w1.b.x + first.x + 10, w1.b.y + first.y + 10],
    async () => {
      hovering.ghost = await page.evaluate(() => !!document.querySelector('#tab-list .tab-item.incoming'));
      hovering.opacity = await app.evaluate(({ BrowserWindow }, id) => BrowserWindow.fromId(id).getOpacity(), w2.id);
    },
  );
  assert.equal(hovering.ghost, true, 'over the strip, the tab is drawn in a gap');
  assert.equal(hovering.opacity, 0, 'and its window hides');
  assert.deepEqual(
    carried(r2.moved),
    [carried(r2.during)[0] + 50, carried(r2.during)[1] + 50],
    'a one-tab window moves itself',
  );
  assert.equal(r2.after.length, 1, 'let go over another strip, the emptied window closes');
  assert.equal(await count(page), before + 1, 'and its tab joins that strip');
  assert.equal(
    Number(await page.locator('#tab-list .tab-item:not(.closing)').first().getAttribute('data-id')),
    moving,
    'in the gap it was shown in, not at the end',
  );
  console.log(
    `PASS (${mode}): tear-off window follows the pointer, one-tab window moves, shows in a gap over a strip and lands there`,
  );
} catch (error) {
  console.error(error);
  process.exitCode = 1;
} finally {
  await app.close();
  fixture.close();
  wss.close();
}
