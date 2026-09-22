/**
 * End to end, with the real app and a real local server: a login has to stick.
 * Sign in, lose the server, sign in again (a fresh session replaces the old
 * one), get the server back. The fresh session must survive the reconnect and
 * reach the server. Fakes cannot show this: it once failed because the server's
 * stale copy overwrote the jar on reconnect, and once because an Electron
 * upgrade renamed the cookie event's cause and every sync stopped, silently.
 * Run: npm run test:sync (needs a display, and server/ and ui/ installed).
 */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { _electron as electron } from '../../../ui/node_modules/playwright/index.mjs';

const KEY = 'sync-e2e-key';
const PORT = 3197;
const API = `http://127.0.0.1:${PORT}/api`;
const SERVER_DIR = fileURLToPath(new URL('../../../server/', import.meta.url));
const BROWSER_DIR = fileURLToPath(new URL('../../', import.meta.url));
/** Names the server must not inherit, so it never reaches a live service. */
const BLANKED = [
  'SUPABASE_URL',
  'SUPABASE_SERVICE_ROLE_KEY',
  'SUPABASE_ANON_KEY',
  'DAYTONA_API_KEY',
  'DATABASE_URL',
  'POSTHOG_KEY',
  'POSTHOG_HOST',
  'SLACK_OPS_WEBHOOK_SIGNUPS',
  'SLACK_OPS_WEBHOOK_EVENTS',
];

/** Resolves after `ms`. */
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** Polls `check` until it is truthy, or throws naming what never happened. */
async function until(check, what, timeoutMs = 60_000) {
  for (const deadline = Date.now() + timeoutMs; Date.now() < deadline; await pause(400))
    if (await Promise.resolve(check()).catch(() => false)) return;
  throw new Error(`timed out waiting for ${what}`);
}

/** Calls the server's API as the test key. */
const rest = (path) => fetch(API + path, { headers: { authorization: `Bearer ${KEY}` } }).then((r) => r.json());

/** The hermetic server's environment: no .env, a scratch data directory, one key. */
function serverEnv(dataDir) {
  const env = { ...process.env, PORT: String(PORT), OYA_DATA_DIR: dataDir, API_KEYS: KEY };
  Object.assign(env, { OYA_PROFILE_SECRET: 'e'.repeat(64), DOTENV_CONFIG_PATH: join(tmpdir(), 'oya-no-env') });
  for (const name of BLANKED) delete env[name];
  return env;
}

/** Starts the server on `dataDir` and resolves once it is healthy. */
async function startServer(dataDir) {
  const child = spawn('node', ['src/index.ts'], { cwd: SERVER_DIR, env: serverEnv(dataDir), stdio: 'ignore' });
  await until(() => fetch(`${API}/health`).then((r) => r.ok), 'the server to start');
  return child;
}

/** Stops the server and waits for it to be gone. */
async function stopServer(child) {
  child.kill('SIGTERM');
  if (child.exitCode === null) await new Promise((resolve) => child.once('exit', resolve));
}

/** Launches the real app, connected to the local server. */
function launchApp(userData) {
  const env = { OYA_USER_DATA_DIR: userData, OYA_SERVER_URL: `ws://127.0.0.1:${PORT}/ws`, OYA_API_KEY: KEY };
  return electron.launch({
    executablePath: createRequire(import.meta.url)('electron'),
    args: [join(BROWSER_DIR, 'main.js')],
    cwd: BROWSER_DIR,
    env: { ...process.env, ...env, OYA_AUTO_CONNECT: 'true' },
  });
}

/** Loads `url` in the app's tab and returns the cookies the page can read. */
function visit(app, url) {
  return app.evaluate(async ({ BrowserWindow }, target) => {
    const views = BrowserWindow.getAllWindows()[0].getBrowserViews();
    const tab = views.find((v) => !v.webContents.getURL().startsWith('file:')).webContents;
    await tab.loadURL(target);
    return tab.executeJavaScript('document.cookie', true);
  }, url);
}

/** A site whose /login?as=x sets a day-long session cookie. */
async function loginSite() {
  const site = createServer((req, res) => {
    const as = new URL(req.url, 'http://x').searchParams.get('as');
    if (as) res.setHeader('Set-Cookie', `sid=${as}; Max-Age=86400; Path=/`);
    res.end('<!doctype html><title>site</title>ok');
  });
  await new Promise((resolve) => site.listen(0, '127.0.0.1', resolve));
  return { site, url: (path) => `http://127.0.0.1:${site.address().port}${path}` };
}

/** The persona's session for the test site, as the server holds it. */
const serverSid = async () => (await rest('/pool/cookies')).cookies?.find((c) => c.name === 'sid')?.value;
/** Whether the desktop is connected. */
const connected = async () => [].concat(await rest('/browsers')).some((b) => b?.id);

/** The scenario; `s` holds the server child so the outage can replace it. */
async function loginSticks(s, app, url) {
  await until(connected, 'the desktop to connect');
  await pause(3000);
  await visit(app, url('/login?as=first-session'));
  await until(async () => (await serverSid()) === 'first-session', 'the first session to reach the server');
  await stopServer(s.server);
  await visit(app, url('/login?as=fresh-session'));
  s.server = await startServer(s.dataDir);
  await until(connected, 'the desktop to reconnect');
  await pause(4000);
  assert.match(await visit(app, url('/')), /sid=fresh-session/, 'the fresh login survived the reconnect');
  await until(async () => (await serverSid()) === 'fresh-session', 'the fresh session to reach the server');
}

const { site, url } = await loginSite();
const s = { dataDir: await mkdtemp(join(tmpdir(), 'oya-sync-e2e-data-')), server: null };
const userData = await mkdtemp(join(tmpdir(), 'oya-sync-e2e-app-'));
s.server = await startServer(s.dataDir);
const app = await launchApp(userData);
try {
  await loginSticks(s, app, url);
  console.log('Sync e2e: a login made while the server was away survives the reconnect and reaches the server');
} finally {
  await app.close().catch(() => {});
  await stopServer(s.server);
  site.close();
  await Promise.all([s.dataDir, userData].map((dir) => rm(dir, { recursive: true, force: true })));
}
