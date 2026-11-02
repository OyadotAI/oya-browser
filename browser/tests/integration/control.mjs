/**
 * Real Electron check of the control handoff UI against a fake control
 * server: agent control shields the page, a person takes and returns control.
 * Run: npm run test:control. Code passed to page.evaluate runs in the shell page.
 */
/* global window, document, getComputedStyle, innerWidth */
import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { createServer } from 'node:http';
import { WebSocketServer } from 'ws';
import { _electron as electron } from '../../../ui/node_modules/playwright/index.mjs';
/** The persona a real server sends in auth_ok; the desktop refuses sign-in without one. */
const PERSONA = createRequire(import.meta.url)('../../anonymity/fingerprint.js').generateProfile({
  seed: 'control-ui',
  platform: 'MacIntel',
});
const profile = await mkdtemp(join(tmpdir(), 'oya-control-ui-'));
await writeFile(join(profile, 'config.json'), JSON.stringify({ ui: { importOffered: true } }));
const fixture = createServer((_req, res) => res.end('<!doctype html><input id="field" autofocus>'));
await new Promise((resolve) => fixture.listen(0, '127.0.0.1', resolve));
const fixtureUrl = `http://127.0.0.1:${fixture.address().port}/`;
const wss = new WebSocketServer({ port: 0, host: '127.0.0.1' });
await new Promise((resolve) => wss.once('listening', resolve));
let state = { mode: 'agent', mine: false, revision: 1 },
  socket,
  app,
  rejectNext = false,
  nextCommand = 0;
/** Commands sent as the server's agent, waiting for their results. */
const waiting = new Map();
wss.on('connection', (ws) => {
  socket = ws;
  ws.on('message', (raw) => {
    const message = JSON.parse(raw);
    if (message.type === 'auth')
      ws.send(JSON.stringify({ type: 'auth_ok', browser_id: 'control-ui', control: state, fingerprint: PERSONA }));
    if (message.type === 'ping') ws.send(JSON.stringify({ type: 'pong' }));
    if (message.type === 'cmd_result') waiting.get(message.id)?.(message);
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
  const page = await app.firstWindow();
  page.setDefaultTimeout(6000);
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  // First run opens on the checklist; an Oya key is under its "API key" tab.
  await page.locator('#tab-key').click();
  await page.locator('#btn-manual').click();
  await page.locator('#cfg-key').fill('test');
  await page.locator('#btn-connect').click();
  // Connected with no agent at work: the person is using the browser, so Human Mode shows.
  await page.locator('#control-action[aria-pressed="true"]').waitFor();
  const views = () =>
    app.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows()[0]
        .getBrowserViews()
        .map((view) => ({ url: view.webContents.getURL(), bounds: view.getBounds() })),
    );
  await page.waitForFunction(() => document.body.classList.contains('mode-browsing'));
  // Browsing opens on the Oya start page, which is never shielded: nothing of a site is there to protect.
  await page.locator('#start-page').waitFor();
  assert.equal((await views()).length, 0, 'the start page is the shell itself: no page view, no shield');
  // Agent mode alone fences nothing: a connected desktop sits in it even when no agent runs.
  assert.notEqual(await page.locator('#url-bar').getAttribute('readonly'), '', 'idle agent mode leaves the page open');
  // The agent starts on the shown tab with a real command, which makes that tab its own.
  const loaded = await command('navigate', { url: fixtureUrl });
  assert.equal(loaded.ok, true, loaded.error);
  await page.waitForFunction(() => document.getElementById('start-page').hidden);
  assert.equal((await views()).length, 2, 'native input shield sits above the page');
  assert.equal(await page.locator('#url-bar').getAttribute('readonly'), '');
  const rejected = await page.evaluate(() =>
    window.oyaBrowser.navigate('https://example.com').then(
      () => false,
      () => true,
    ),
  );
  assert(rejected, 'IPC enforces navigation ownership');
  await personBesideAgent(page, views);
  // Switching to Human Mode from a working agent asks first; the person answers "Human Mode".
  await app.evaluate(({ dialog }) => {
    globalThis.shownDialogs = [];
    dialog.showMessageBox = async (_window, options) => {
      globalThis.shownDialogs.push(options);
      return { response: 1, checkboxChecked: false };
    };
  });
  await page.locator('#control-action').click();
  await page.locator('#control-action[aria-pressed="true"]').waitFor();
  assert.equal((await views()).length, 1, 'taking control removes the native shield');
  assert.equal(await page.locator('#control-agent').isEnabled(), true, 'AI Mode is on offer to hand back');
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
  await page.locator('#control-agent').click();
  // Handed back, the page stays the person's until the agent acts: Human Mode shows.
  await waitFor(async () => (await page.evaluate(() => window.oyaBrowser.getControlState())).mode === 'agent');
  await page.locator('#control-action[aria-pressed="true"]').waitFor();
  // Released to the agent, its next command takes the page back, and AI Mode shows.
  assert.equal((await command('run_script', { script: 'return 1' })).ok, true);
  await page.locator('#control-agent[aria-pressed="true"]').waitFor();
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
  // A refused takeover is explained in a native error dialog, with the server's reason.
  await expectDialog(app, 'Another operator has control');
  assert.equal((await views()).length, 2, 'failed takeover stays watch-only');
  state = { mode: 'human', mine: false, expiresAt: Date.now() + 300000, revision: state.revision + 1 };
  socket.send(JSON.stringify({ type: 'control_mode', mode: state.mode, state }));
  await page.waitForFunction(() => document.querySelector('.control-switch').hidden);
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
    'Desktop control UI passed: per-tab agent control, handoff, native input gating, dialogs, reduced motion, compact layouts, conflicts, stale updates and disconnect.',
  );
} finally {
  await app?.close();
  for (const ws of wss.clients) ws.terminate();
  await new Promise((resolve) => wss.close(resolve));
  await new Promise((resolve) => fixture.close(resolve));
  await rm(profile, { recursive: true, force: true });
}

/** Sends one command as the server's agent would, and answers its result. */
function command(action, params = {}) {
  const id = `c${nextCommand++}`;
  const answered = new Promise((resolve) => waiting.set(id, resolve));
  socket.send(JSON.stringify({ type: 'cmd', id, action, params }));
  return answered;
}

/** The address of the shown view's page, and how many views the window holds. */
async function shownPage(application) {
  return application.evaluate(({ BrowserWindow }) => {
    const views = BrowserWindow.getAllWindows()[0].getBrowserViews();
    return { top: views.at(-1)?.webContents.getURL(), count: views.length };
  });
}

/**
 * While the agent drives its tab, the person opens another and uses it: no shield there,
 * keys reach it, the address bar works; the agent keeps acting on its own tab, still
 * painting beneath; the agent's tab stays fenced and cannot be closed by the person.
 */
async function personBesideAgent(page, views) {
  const mine = `${fixtureUrl}mine`;
  await page.evaluate((url) => window.oyaBrowser.newTab(url), mine);
  await page.waitForFunction(() => !document.getElementById('url-bar').hasAttribute('readonly'));
  await waitFor(async () => (await shownPage(app)).top === mine);
  // On their own tab the person sees Human Mode, and can still choose it to take over from the agent.
  await page.locator('#control-action[aria-pressed="true"]').waitFor();
  assert.equal(await page.locator('#control-action').isEnabled(), true, 'Human Mode still takes over');
  const stack = await views();
  assert.deepEqual(
    stack.map((v) => v.url),
    [fixtureUrl, mine],
    "the agent's page sits beneath, no shield",
  );
  assert.deepEqual(stack[0].bounds, stack[1].bounds, 'beneath with the same bounds, so it keeps painting');
  const typed = await app.evaluate(async ({ BrowserWindow }) => {
    const view = BrowserWindow.getAllWindows()[0].getBrowserViews().at(-1);
    view.webContents.focus();
    await view.webContents.executeJavaScript('document.querySelector("input").focus()');
    let prevented = false;
    view.webContents.once('before-input-event', (event) => setImmediate(() => (prevented = event.defaultPrevented)));
    view.webContents.sendInputEvent({ type: 'keyDown', keyCode: 'M' });
    view.webContents.sendInputEvent({ type: 'char', keyCode: 'm' });
    view.webContents.sendInputEvent({ type: 'keyUp', keyCode: 'M' });
    await new Promise((resolve) => setTimeout(resolve, 100));
    return { prevented, value: await view.webContents.executeJavaScript('document.querySelector("input").value') };
  });
  assert.deepEqual(typed, { prevented: false, value: 'm' }, "the person's keys reach their own tab");
  const where = await command('run_script', { script: 'return location.href' });
  assert.equal(where.data?.value, fixtureUrl, 'the agent keeps acting on its own tab');
  const shot = await command('screenshot', { format: 'jpeg' });
  assert.ok(shot.ok && shot.data.screenshot.length > 1000, "the agent's hidden tab still captures");
  const tabs = (await command('list_tabs')).data.tabs;
  const agentTab = tabs.find((t) => t.active);
  const myTab = tabs.find((t) => !t.active);
  assert.equal(agentTab.url, fixtureUrl, "list_tabs marks the agent's tab active");
  const refusedSwitch = await command('switch_tab', { tab_id: myTab.id });
  assert.match(refusedSwitch.error, /person is using that tab/);
  const closeAgentTab = await page.evaluate(
    (id) =>
      window.oyaBrowser.closeTab(id).then(
        () => 'closed',
        (e) => e.message,
      ),
    agentTab.id,
  );
  assert.match(closeAgentTab, /Switch to Human Mode/, "the person cannot close the agent's tab");
  await page.evaluate((id) => window.oyaBrowser.activateTab(id), agentTab.id);
  await waitFor(async () => (await shownPage(app)).top?.includes('control-shield'));
  await page.waitForFunction(() => document.getElementById('url-bar').hasAttribute('readonly'));
  // Looking at the agent's tab, the person clicks + : a tab of their own opens, as in any browser.
  const before = (await command('list_tabs')).data.tabs.length;
  await page.locator('#btn-new-tab').click();
  await waitFor(async () => (await command('list_tabs')).data.tabs.length === before + 1);
  await page.waitForFunction(() => !document.getElementById('url-bar').hasAttribute('readonly'));
  const opened = (await command('list_tabs')).data.tabs.at(-1);
  await page.evaluate((id) => window.oyaBrowser.closeTab(id), opened.id);
  await page.evaluate((id) => window.oyaBrowser.activateTab(id), agentTab.id);
  await page.evaluate((id) => window.oyaBrowser.closeTab(id), myTab.id).catch(() => {});
  await page.evaluate((id) => window.oyaBrowser.activateTab(id), agentTab.id);
  await waitFor(async () => (await shownPage(app)).count === 2);
}

/** Polls `check` until it holds, failing after a few seconds. */
async function waitFor(check) {
  for (let i = 0; i < 100; i++) {
    if (await check()) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error('Condition not met in time');
}

/** Waits until the app showed a native dialog whose detail carries `text`. */
async function expectDialog(application, text) {
  for (let i = 0; i < 100; i++) {
    const shown = await application.evaluate(() => globalThis.shownDialogs.map((d) => d.detail || ''));
    if (shown.some((detail) => detail.includes(text))) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`No dialog said "${text}"`);
}
