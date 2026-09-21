/**
 * Real Electron check of the control handoff UI against a fake control
 * server: agent control shields the page, a person takes and returns control.
 * Run: npm run test:control. Code passed to page.evaluate runs in the shell page.
 */
/* global window, document, getComputedStyle, innerWidth */
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { createServer } from 'node:http';
import { WebSocketServer } from 'ws';
import { _electron as electron } from '../../../ui/node_modules/playwright/index.mjs';
const profile = await mkdtemp(join(tmpdir(), 'oya-control-ui-'));
const fixture = createServer((_req, res) => res.end('<!doctype html><input id="field" autofocus>'));
await new Promise((resolve) => fixture.listen(0, '127.0.0.1', resolve));
const fixtureUrl = `http://127.0.0.1:${fixture.address().port}/`;
const wss = new WebSocketServer({ port: 0, host: '127.0.0.1' });
await new Promise((resolve) => wss.once('listening', resolve));
let state = { mode: 'agent', mine: false, revision: 1 },
  socket,
  app,
  rejectNext = false;
wss.on('connection', (ws) => {
  socket = ws;
  ws.on('message', (raw) => {
    const message = JSON.parse(raw);
    if (message.type === 'auth') ws.send(JSON.stringify({ type: 'auth_ok', browser_id: 'control-ui', control: state }));
    if (message.type === 'ping') ws.send(JSON.stringify({ type: 'pong' }));
    if (message.type !== 'desktop_control') return;
    if (rejectNext) {
      rejectNext = false;
      ws.send(
        JSON.stringify({
          type: 'desktop_control_result',
          id: message.id,
          error: 'Another operator has control',
          state,
        }),
      );
      return;
    }
    const modes = { request: 'paused', acquire: 'human', return: 'agent', renew: 'human' };
    state = {
      mode: modes[message.action],
      mine: message.action !== 'return',
      expiresAt: Date.now() + 300000,
      revision: state.revision + 1,
    };
    setTimeout(
      () => ws.readyState === 1 && ws.send(JSON.stringify({ type: 'desktop_control_result', id: message.id, state })),
      60,
    );
  });
});
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
  page.setDefaultTimeout(6000);
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await app.evaluate(({ app }, url) => {
    app.on('web-contents-created', (_event, contents) =>
      contents.session.webRequest.onBeforeRequest(
        { urls: ['https://google.com/*', 'https://www.google.com/*'] },
        (_details, callback) => callback({ redirectURL: url }),
      ),
    );
  }, fixtureUrl);
  await page.locator('#btn-manual').click();
  await page.locator('#cfg-key').fill('test');
  await page.locator('#btn-connect').click();
  await page.getByText('Agent control', { exact: true }).waitFor();
  const views = () =>
    app.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows()[0]
        .getBrowserViews()
        .map((view) => ({ url: view.webContents.getURL(), bounds: view.getBounds() })),
    );
  await page.waitForFunction(() => document.body.classList.contains('mode-browsing'));
  assert.equal((await views()).length, 2, 'native input shield sits above the page');
  assert.equal(await page.locator('#url-bar').getAttribute('readonly'), '');
  const rejected = await page.evaluate(() =>
    window.oyaBrowser.navigate('https://example.com').then(
      () => false,
      () => true,
    ),
  );
  assert(rejected, 'IPC enforces navigation ownership');
  await page.locator('#control-action').click();
  await page.getByText('You’re in control', { exact: true }).waitFor();
  assert.equal((await views()).length, 1, 'taking control removes the native shield');
  assert.equal(await page.locator('#control-action').innerText(), 'Release to agent');
  await app.evaluate(async ({ app, BrowserWindow }) => {
    app.focus({ steal: true });
    const window = BrowserWindow.getAllWindows()[0];
    window.focus();
    const view = window.getBrowserView();
    globalThis.controlInputEvents = [];
    view.webContents.on('before-input-event', (event, input) =>
      globalThis.controlInputEvents.push({ type: input.type, prevented: event.defaultPrevented }),
    );
    const deadline = Date.now() + 6000;
    while ((!view.webContents.getURL().startsWith('http:') || view.webContents.isLoading()) && Date.now() < deadline)
      await new Promise((resolve) => setTimeout(resolve, 20));
    view.webContents.focus();
    await new Promise((resolve) => setTimeout(resolve, 100));
    await view.webContents.executeJavaScript(
      'document.querySelector("input").focus(); window.humanKeys = 0; document.addEventListener("keydown", () => window.humanKeys++)',
    );
    view.webContents.sendInputEvent({ type: 'keyDown', keyCode: 'H' });
    view.webContents.sendInputEvent({ type: 'keyUp', keyCode: 'H' });
    await view.webContents.insertText('h');
  });
  assert.equal(
    await app.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows()[0]
        .getBrowserView()
        .webContents.executeJavaScript('document.querySelector("input").value'),
    ),
    'h',
  );
  assert(
    (await app.evaluate(() => globalThis.controlInputEvents)).every((event) => !event.prevented),
    'human native input is not intercepted',
  );
  await page.locator('#control-action').click();
  await page.getByText('Agent control', { exact: true }).waitFor();
  await app.evaluate(async ({ BrowserWindow }) => {
    const view = BrowserWindow.getAllWindows()[0]
      .getBrowserViews()
      .find((v) => v.webContents.getURL().startsWith('http:'));
    globalThis.controlInputEvents = [];
    await view.webContents.executeJavaScript(
      'window.keys = 0; document.addEventListener("keydown", () => window.keys++)',
    );
    view.webContents.focus();
    view.webContents.sendInputEvent({ type: 'keyDown', keyCode: 'X' });
    view.webContents.sendInputEvent({ type: 'keyUp', keyCode: 'X' });
  });
  assert.equal(
    await app.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows()[0]
        .getBrowserViews()
        .find((v) => v.webContents.getURL().startsWith('http:'))
        .webContents.executeJavaScript('window.keys'),
    ),
    0,
    'native keyboard events cannot reach the agent-controlled page',
  );
  assert(
    (await app.evaluate(() => globalThis.controlInputEvents)).every((event) => event.prevented),
    'agent native input is intercepted',
  );
  // Dialogs remain accessible above the native page and input shield.
  await page.locator('#btn-commands').click();
  await page.locator('#shell-overlay').waitFor({ state: 'visible' });
  assert(await page.locator('#page-backdrop').evaluate((el) => !el.hidden && el.naturalWidth > 0));
  assert.equal((await views()).length, 0);
  await page.keyboard.press('Escape');
  assert.equal((await views()).length, 2);
  await page.emulateMedia({ reducedMotion: 'reduce' });
  assert.equal(await page.locator('.control-orbit').evaluate((el) => getComputedStyle(el).animationName), 'none');
  for (const width of [1280, 800, 600]) {
    await app.evaluate(
      ({ BrowserWindow }, width) => BrowserWindow.getAllWindows()[0].setContentSize(width, 600),
      width,
    );
    await page.waitForFunction((width) => innerWidth === width, width);
    const rect = await page.locator('#control-action').boundingBox();
    assert(rect.x >= 0 && rect.x + rect.width <= width, 'handoff remains visible at compact widths');
  }
  rejectNext = true;
  await page.locator('#control-action').click();
  await page.getByText('Another operator has control', { exact: true }).waitFor();
  assert.equal((await views()).length, 2, 'failed takeover stays watch-only');
  state = { mode: 'human', mine: false, expiresAt: Date.now() + 300000, revision: state.revision + 1 };
  socket.send(JSON.stringify({ type: 'control_mode', mode: state.mode, state }));
  await page.waitForFunction(() => document.getElementById('control-action').hidden);
  socket.send(
    JSON.stringify({
      type: 'control_mode',
      mode: 'human',
      state: { mode: 'human', mine: true, expiresAt: Date.now() + 300000, revision: 1 },
    }),
  );
  await page.waitForTimeout(30);
  assert.equal((await views()).length, 2, 'stale ownership update cannot remove shield');
  socket.close(4000);
  await page.getByText('Disconnected', { exact: true }).waitFor();
  assert.equal((await views()).length, 1);
  assert.deepEqual(errors, []);
  console.log(
    'Desktop control UI passed: handoff, native input gating, dialogs, reduced motion, compact layouts, conflicts, stale updates and disconnect.',
  );
} finally {
  await app?.close();
  for (const ws of wss.clients) ws.terminate();
  await new Promise((resolve) => wss.close(resolve));
  await new Promise((resolve) => fixture.close(resolve));
  await rm(profile, { recursive: true, force: true });
}
