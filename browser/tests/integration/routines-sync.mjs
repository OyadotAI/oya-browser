/**
 * Real Electron check that routines belong to the project on the server,
 * against a real local server: a routine the app kept locally before the move
 * is handed to the project, a routine made elsewhere appears in the pane by
 * itself, a run another browser claimed shows as running elsewhere (with no
 * Run now), the switch turns a routine off on the server, and Run now claims
 * and records a run as this browser. Screenshots of the pane land in the temp
 * folder. Run: npm run test:routines-sync. Needs no network.
 */
/* global document, window */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { _electron as electron } from '../../../ui/node_modules/playwright/index.mjs';

const KEY = 'routines-sync-key';
const PORT = 7000 + Math.floor(Math.random() * 1000);
const BASE = `http://127.0.0.1:${PORT}`;
const serverDir = fileURLToPath(new URL('../../../server/', import.meta.url));
const HOURLY = { kind: 'every', n: 1, unit: 'hours' };
/** A routine as the app kept it in config.json before routines moved to the server. */
const LEGACY = { id: 'legacy-1', name: 'Morning inbox', prompt: 'Check mail', schedule: HOURLY, enabled: true };

/** Calls the server as the project, as another desktop or the console would. */
async function api(method, path, body) {
  const res = await fetch(`${BASE}/api/${path}`, {
    method,
    headers: { Authorization: `Bearer ${KEY}`, 'Content-Type': 'application/json' },
    ...(body ? { body: JSON.stringify(body) } : {}),
  });
  const json = await res.json();
  assert.ok(res.ok, `${method} ${path}: ${res.status} ${JSON.stringify(json)}`);
  return json;
}

/** Waits until `check` returns true, or fails with `what`. */
async function until(check, what, ms = 15000) {
  const deadline = Date.now() + ms;
  while (Date.now() < deadline) {
    if (await check().catch(() => false)) return;
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(`Timed out waiting for ${what}`);
}

/** Saves a screenshot of the Agent panel as oya-routines-<name>.png in the temp folder. */
const shot = (page, name) =>
  page.locator('#dev-panel').screenshot({ path: join(tmpdir(), `oya-routines-${name}.png`), animations: 'disabled' });

/** The card for routine `name`. */
const card = (page, name) => page.locator('.routine-card', { has: page.locator('.routine-name', { hasText: name }) });

const dataDir = await mkdtemp(join(tmpdir(), 'oya-routines-server-'));
const serverEnv = { ...process.env, PORT: String(PORT), OYA_STORAGE: 'file', OYA_DATA_DIR: dataDir, API_KEYS: KEY };
for (const name of ['SUPABASE_URL', 'SUPABASE_SERVICE_KEY', 'DATABASE_URL', 'OPENAI_API_KEY', 'DAYTONA_API_KEY'])
  delete serverEnv[name];
Object.assign(serverEnv, { OYA_OPENROUTER_MODELS_URL: '', DOTENV_CONFIG_PATH: join(dataDir, 'no-env') });
const server = spawn(process.execPath, ['src/index.ts'], { cwd: serverDir, env: serverEnv, stdio: 'ignore' });
let app;
try {
  await until(async () => (await fetch(`${BASE}/health`)).ok, 'the server');
  const profile = await mkdtemp(join(tmpdir(), 'oya-routines-app-'));
  await writeFile(join(profile, 'config.json'), JSON.stringify({ routines: [LEGACY] }));
  app = await electron.launch({
    executablePath: createRequire(import.meta.url)('electron'),
    args: [fileURLToPath(new URL('../../main.js', import.meta.url))],
    cwd: fileURLToPath(new URL('../../', import.meta.url)),
    env: {
      ...process.env,
      OYA_USER_DATA_DIR: profile,
      OYA_API_KEY: KEY,
      OYA_SERVER_URL: `ws://127.0.0.1:${PORT}/ws`,
      OYA_REMOTE_DEBUGGING_PORT: '0',
    },
  });
  const page = await app.firstWindow();
  page.setDefaultTimeout(8000);
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.waitForFunction(() => document.body.classList.contains('mode-browsing'));
  await page.locator('#pane-chat').waitFor({ state: 'visible' });
  await page.waitForFunction(() => document.documentElement.dataset.panelMoving === 'false');

  // 1. The routine kept locally is handed to the project.
  await until(async () => (await api('GET', 'routines')).routines.some((r) => r.id === 'legacy-1'), 'the hand-over');
  await page.locator('[data-pane="routines"]').click();
  await card(page, 'Morning inbox').waitFor();

  // 2. A routine made elsewhere shows up by itself.
  const made = await api('POST', 'routines', { name: 'Price watch', prompt: 'Check prices', schedule: HOURLY });
  await card(page, 'Price watch').waitFor();
  await shot(page, 'list');

  // 3. Another browser's run shows as running elsewhere, with no Run now.
  await api('POST', `routines/${made.id}/claim`, { lastRunAt: null, runId: 'elsewhere-1', browserId: 'other-desktop' });
  await card(page, 'Price watch').locator('.routine-status', { hasText: 'Running on another Oya browser' }).waitFor();
  assert.equal(await card(page, 'Price watch').locator('.routine-action').count(), 0, 'no Run now');
  await shot(page, 'elsewhere');
  await api('PATCH', `routines/${made.id}/runs/elsewhere-1`, {
    status: 'done',
    result: 'Cheapest: $4',
    steps: ['navigate'],
  });
  await card(page, 'Price watch').locator('.run-status', { hasText: 'Done' }).waitFor();

  // 4. The switch turns the routine off on the server.
  await card(page, 'Morning inbox').locator('.switch').click();
  await until(
    async () => (await api('GET', 'routines')).routines.find((r) => r.id === 'legacy-1').enabled === false,
    'the switch to reach the server',
  );
  await card(page, 'Morning inbox').locator('.routine-status', { hasText: 'Off' }).waitFor();

  // 5. Run now claims as this browser and records the run (no model key here, so it fails, and says why).
  const browserId = await page.evaluate(() => window.oyaBrowser.getStatus().then((s) => s.browserId));
  await card(page, 'Price watch').locator('.routine-action').click();
  await until(async () => {
    const run = (await api('GET', 'routines')).routines.find((r) => r.id === made.id).runs[0];
    return run.by === browserId && run.status !== 'running';
  }, 'the run to be recorded');
  await card(page, 'Price watch').locator('.routine-history-toggle').click();
  await card(page, 'Price watch').locator('.routine-run details[open]').waitFor();
  await shot(page, 'history');
  await app.evaluate(({ nativeTheme }) => (nativeTheme.themeSource = 'dark'));
  await page.waitForFunction(() => document.documentElement.dataset.theme === 'dark');
  await card(page, 'Price watch').locator('.routine-more').click();
  await shot(page, 'menu-dark');
  assert.deepEqual(errors, [], 'no page errors');
  console.log('routines-sync: ok');
} finally {
  await app?.close().catch(() => {});
  server.kill();
}
