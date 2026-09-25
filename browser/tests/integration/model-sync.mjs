/**
 * Real Electron check that the Ask pane's model card and the server agree,
 * both ways, against a real local server: a model set elsewhere (as the web
 * console sets it) shows in the card, a model saved in the card reaches the
 * server without losing the key, and a change made while the card is closed
 * is pushed to the desktop. Run: npm run test:model-sync. Needs no network.
 */
/* global window, document, ChatModel */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { _electron as electron } from '../../../ui/node_modules/playwright/index.mjs';

const KEY = 'model-sync-key';
const PORT = 3000 + Math.floor(Math.random() * 1000) + 4000;
const BASE = `http://127.0.0.1:${PORT}`;
const serverDir = fileURLToPath(new URL('../../../server/', import.meta.url));

/** The key's settings, as the console reads them. */
const config = async () => (await fetch(`${BASE}/api/config`, { headers: { Authorization: `Bearer ${KEY}` } })).json();

/** Saves settings, as the console does. */
const setConfig = async (body) => {
  const res = await fetch(`${BASE}/api/config`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${KEY}`, 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  assert.equal(res.status, 200, await res.text());
};

/** Saves a screenshot of the Ask panel as oya-model-sync-<name>.png in the temp folder. */
const shot = (page, name) =>
  page.locator('#dev-panel').screenshot({ path: join(tmpdir(), `oya-model-sync-${name}.png`), animations: 'disabled' });

/** Waits until `check` returns true, or fails with `what`. */
async function until(check, what, ms = 15000) {
  const deadline = Date.now() + ms;
  while (Date.now() < deadline) {
    if (await check().catch(() => false)) return;
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(`Timed out waiting for ${what}`);
}

const dataDir = await mkdtemp(join(tmpdir(), 'oya-model-sync-server-'));
const serverEnv = { ...process.env, PORT: String(PORT), OYA_STORAGE: 'file', OYA_DATA_DIR: dataDir, API_KEYS: KEY };
for (const name of ['SUPABASE_URL', 'SUPABASE_SERVICE_KEY', 'DATABASE_URL', 'OPENAI_API_KEY', 'DAYTONA_API_KEY'])
  delete serverEnv[name];
Object.assign(serverEnv, { OYA_OPENROUTER_MODELS_URL: '', DOTENV_CONFIG_PATH: join(dataDir, 'no-env') });
const server = spawn(process.execPath, ['src/index.ts'], { cwd: serverDir, env: serverEnv, stdio: 'ignore' });
let app;
try {
  await until(async () => (await fetch(`${BASE}/health`)).ok, 'the server');
  // The console's move: OpenAI, its own key, GPT-4.1.
  await setConfig({ llm_provider: 'openai', openai_api_key: 'sk-console-1234', chat_model: 'gpt-4.1' });

  app = await electron.launch({
    executablePath: createRequire(import.meta.url)('electron'),
    args: [fileURLToPath(new URL('../../main.js', import.meta.url))],
    cwd: fileURLToPath(new URL('../../', import.meta.url)),
    env: {
      ...process.env,
      OYA_USER_DATA_DIR: await mkdtemp(join(tmpdir(), 'oya-model-sync-')),
      OYA_API_KEY: KEY,
      OYA_SERVER_URL: `ws://127.0.0.1:${PORT}/ws`,
      OYA_REMOTE_DEBUGGING_PORT: '0',
    },
  });
  const page = await app.firstWindow();
  page.setDefaultTimeout(8000);
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await until(
    () => page.evaluate(() => window.oyaBrowser.getStatus().then((s) => !!s.connected)),
    'the desktop to connect',
  );
  // Ask opens by itself once pages show (clicking the Agent button would close it): wait for it to settle.
  await page.waitForFunction(() => document.body.classList.contains('mode-browsing'));
  await page.locator('#pane-chat').waitFor({ state: 'visible' });
  await page.waitForFunction(() => document.documentElement.dataset.panelMoving === 'false');
  await page.locator('#chat-model-open').waitFor({ state: 'visible' });

  // 1. Server to desktop: the card opens on what the console chose, not on "Claude".
  await page.locator('#chat-model-open').click();
  await page.locator('#chat-model').waitFor({ state: 'visible' });
  await page.waitForFunction(() => document.documentElement.dataset.panelMoving === 'false');
  assert.equal(
    await page.locator('#chat-model-provider [aria-checked="true"]').getAttribute('data-provider'),
    'openai',
  );
  assert.equal(await page.locator('#chat-model-name').textContent(), 'GPT-4.1');
  const providers = await page.locator('#chat-model-provider .provider-name').allTextContents();
  assert.ok(providers.includes('OpenRouter'), `OpenRouter offered: ${providers}`);
  await shot(page, 'card');
  await page.locator('#chat-model-model').click();
  await shot(page, 'menu');

  // 2. Desktop to server: a new model, no key typed. The key and endpoint must survive.
  await page.locator('#chat-model-search').fill('4o mini');
  await page.locator('#chat-model-search').press('Enter');
  await page.locator('#chat-model-save').click();
  await page.locator('#chat-model').waitFor({ state: 'hidden' });
  const saved = await config();
  assert.equal(saved.llm_provider, 'openai');
  assert.equal(saved.chat_model, 'gpt-4o-mini');
  assert.equal(saved.openai_api_key, '••••1234', 'the key typed in the console is kept');
  assert.equal(saved.effective.hasLlmKey, true);

  // 3. Server to desktop again, with the card closed: the push reaches the desktop by itself.
  await setConfig({ llm_provider: 'openai', chat_model: 'gpt-6-sol' });
  await until(
    () => page.evaluate(() => ChatModel.status.model === 'gpt-6-sol'),
    'the pushed change to reach the desktop',
  );
  await page.locator('#chat-model-open').click();
  assert.equal(await page.locator('#chat-model-name').textContent(), 'GPT-6 Sol');

  // 4. Switching provider in the desktop asks for that provider's key, and then takes it.
  await page.locator('#chat-model-provider [data-provider="openrouter"]').click();
  assert.equal(await page.locator('#chat-model-name').textContent(), 'Claude Sonnet 5');
  await page.locator('#chat-model-model').click();
  await page.locator('#chat-model-search').fill('claude');
  await shot(page, 'search');
  await app.evaluate(({ nativeTheme }) => (nativeTheme.themeSource = 'dark'));
  await page.waitForFunction(() => document.documentElement.dataset.theme === 'dark');
  await page.locator('#chat-model-search').fill('');
  await shot(page, 'menu-dark');
  await page.locator('#chat-model-search').press('Escape');
  await shot(page, 'card-dark');
  await page.locator('#chat-model-save').click();
  await page.locator('#chat-model-error').waitFor({ state: 'visible' });
  await page.locator('#chat-model-key').fill('sk-or-test-5678');
  await page.locator('#chat-model-save').click();
  await page.locator('#chat-model').waitFor({ state: 'hidden' });
  const switched = await config();
  assert.deepEqual(
    [switched.llm_provider, switched.chat_model, switched.openai_api_key, switched.effective.baseUrl],
    ['openrouter', 'anthropic/claude-sonnet-5', '••••5678', 'https://openrouter.ai/api/v1'],
  );
  assert.deepEqual(errors, [], 'no page errors');
  console.log('model-sync: ok');
} finally {
  await app?.close().catch(() => {});
  server.kill();
}
