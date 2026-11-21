/**
 * End to end, with the real app and a real local server: every way this browser
 * comes to act for an account, and that it then says the right one. Two keys are
 * two projects on one server, the way two people on oyabrowser.com are. Fakes
 * cannot show this: the shell suite answers `get-account` itself, so a pairing
 * that swapped the key while the account page kept naming the previous person
 * passed every check.
 * Flows: a cancelled link, a first pairing, pairing as someone else with the
 * account page open, a used code, links that are not pairing links, the app's own
 * Sign in (loopback return, good and forged), a typed key, Log out, a remembered
 * account, a restart, and a launch with the server unreachable.
 * Run: npm run test:pairing (needs a display, and server/ and ui/ installed).
 */
/* global window */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { _electron as electron } from '../../../ui/node_modules/playwright/index.mjs';

/** The two accounts: each key is its own project. */
const KEYS = { a: 'pairing-e2e-key-a', b: 'pairing-e2e-key-b' };
const PORT = 3196;
/** A port of its own for the app's MCP server: the person's own Oya may hold the usual one. */
const MCP_PORT = 47_396;
const HTTP = `http://127.0.0.1:${PORT}`;
const API = `${HTTP}/api`;
const WS = `ws://127.0.0.1:${PORT}/ws`;
const SERVER_DIR = fileURLToPath(new URL('../../../server/', import.meta.url));
const BROWSER_DIR = fileURLToPath(new URL('../../', import.meta.url));
/** Names the server must not inherit, so it never reaches a live service. */
const BLANKED = [
  'SUPABASE_URL',
  'SUPABASE_SERVICE_ROLE_KEY',
  'SUPABASE_ANON_KEY',
  'DAYTONA_API_KEY',
  'DATABASE_URL',
  'OYA_STORAGE',
  'POSTHOG_KEY',
  'POSTHOG_HOST',
  'SLACK_OPS_WEBHOOK_SIGNUPS',
  'SLACK_OPS_WEBHOOK_EVENTS',
  'SLACK_OPS_WEBHOOK_PRODUCT',
];
/** Long enough for a wrong change to have shown itself. */
const SETTLE_MS = 2500;

/** Resolves after `ms`. */
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** Polls `check` until it is truthy, or throws naming what never happened. */
async function until(check, what, timeoutMs = 45_000) {
  for (const deadline = Date.now() + timeoutMs; Date.now() < deadline; await pause(300))
    if (await Promise.resolve(check()).catch(() => false)) return;
  throw new Error(`timed out waiting for ${what}`);
}

/** Calls the server's API as `key`: a GET, or `method` with `body`. */
async function rest(key, path, body, method = 'POST') {
  const headers = { authorization: `Bearer ${key}`, 'content-type': 'application/json' };
  const init = body ? { method, headers, body: JSON.stringify(body) } : { headers };
  return (await fetch(API + path, init)).json();
}

/** The hermetic server's environment: no .env, a scratch data directory, the two keys. */
function serverEnv(dataDir) {
  const env = { ...process.env, PORT: String(PORT), OYA_DATA_DIR: dataDir, API_KEYS: Object.values(KEYS).join(',') };
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

/**
 * Launches the real app, signed out, on `userData`. Its dialogs are answered from
 * `globalThis.answer` and recorded, and pages it would open in another browser are
 * recorded instead, so nothing reaches the screen or the person's own browser.
 */
async function launchApp(userData) {
  const env = { OYA_USER_DATA_DIR: userData, OYA_SERVER_URL: WS, OYA_API_KEY: '', OYA_AUTO_CONNECT: 'false' };
  const app = await electron.launch({
    executablePath: createRequire(import.meta.url)('electron'),
    args: [BROWSER_DIR],
    cwd: BROWSER_DIR,
    env: { ...process.env, ...env, OYA_MCP_PORT: String(MCP_PORT) },
  });
  await app.evaluate(installSeams);
  return { app, page: await shellPage(app) };
}

/** The shell's own page. A signed-in launch opens its tab first, so the first window is not it. */
async function shellPage(app) {
  const shell = () => app.windows().find((w) => w.url().includes('/renderer/index.html'));
  await until(shell, 'the shell window');
  await shell().waitForFunction(() => !!window.oyaBrowser);
  return shell();
}

/** Runs in the main process: the dialog and open-in-a-browser seams. */
function installSeams({ dialog, shell, app }) {
  Object.assign(globalThis, { asked: [], opened: [], answer: { response: 0, checkboxChecked: false } });
  dialog.showMessageBox = async (...args) => {
    globalThis.asked.push(args.find((a) => a && a.message));
    return globalThis.answer;
  };
  app.isDefaultProtocolClient = () => false;
  shell.openExternal = async (url) => void globalThis.opened.push(url);
}

/** Sets what the next dialogs answer: 1 is Connect, 0 is Cancel. */
const answer = (app, response) => app.evaluate((_e, r) => void (globalThis.answer.response = r), response);
/** The dialogs the app has shown so far. */
const asked = (app) => app.evaluate(() => globalThis.asked);
/** The pages the app tried to open in another browser. */
const opened = (app) => app.evaluate(() => globalThis.opened);
/** Sends `link` the way macOS delivers it. */
const sendLink = (app, link) =>
  app.evaluate(({ app: electronApp }, url) => electronApp.emit('open-url', { preventDefault() {} }, url), link);

/** A fresh pairing link for `key`, as the dashboard's Connect desktop mints it. */
async function pairingLink(key) {
  const { code } = await rest(key, '/pairing', { profile: 'default' });
  assert.ok(code, 'the server minted a pairing code');
  return { code, link: `oya://connect?code=${encodeURIComponent(code)}&server=${encodeURIComponent(WS)}` };
}

/** The saved settings, as the shell reads them. */
const config = (page) => page.evaluate(() => window.oyaBrowser.getConfig());
/** Who the main process says this browser is. */
const account = (page) => page.evaluate(() => window.oyaBrowser.getAccount());
/** The project `key` belongs to, from the server itself. */
const projectOf = async (key) => (await rest(key, '/auth/whoami')).project;

/**
 * Gives each project's account a name on its way to the shell. A key from the environment has
 * no person behind it, so the toolbar avatar and the profile menu would say the same thing for
 * both and a stale one could not be seen. The real handler still answers, from the real server
 * and the real remembered copy; only the name is added, by project.
 */
function nameAccounts(app, names) {
  return app.evaluate(({ ipcMain }, byProject) => {
    const real = ipcMain._invokeHandlers.get('get-account');
    ipcMain.removeHandler('get-account');
    ipcMain.handle('get-account', async (...args) => {
      const account = await real(...args);
      return account?.project ? { ...account, name: byProject[account.project.id] } : account;
    });
  }, names);
}

/** The desktop browsers connected under `key`. */
const browsersOf = async (key) => [].concat(await rest(key, '/browsers')).filter((b) => b?.id);

/** Waits until the browser is connected as `key`'s project and no other, and says so with Settings shut. */
async function signedInAs(s, name) {
  await actsFor(s, name);
  await avatarShows(s.page, s.projects[name]);
}

/** Waits until the saved key, the server and the main process all agree the browser is `name`'s. */
async function actsFor(s, name) {
  const other = name === 'a' ? 'b' : 'a';
  await until(async () => (await config(s.page)).apiKey === KEYS[name], `the saved key to be ${name}'s`);
  await until(async () => (await browsersOf(KEYS[name])).length === 1, `the server to see it under ${name}`);
  assert.equal((await browsersOf(KEYS[other])).length, 0, `nothing is left connected under ${other}`);
  await until(async () => (await account(s.page))?.project?.id === s.projects[name].id, `whoami to name ${name}`);
}

/** What the toolbar avatar and the profile menu say, neither of which needs Settings open. */
const shown = async (page) => ({
  avatar: await page.locator('#conn-pill .profile-avatar').textContent(),
  menu: await page.locator('#profiles-overlay .profiles-who strong').textContent(),
});

/** Waits until the avatar and the profile menu name `project`'s account. */
const avatarShows = (page, project) =>
  until(async () => {
    const { avatar, menu } = await shown(page);
    return avatar === project.label[0] && menu === project.label;
  }, `the avatar and profile menu to name ${project.label}`);

/** Opens Settings on the account page, through the profile menu as a person does. */
async function openAccountPage(page) {
  await page.locator('#conn-pill').click();
  await page.locator('#profiles-settings').click();
  await page.locator('#profile-section').waitFor({ state: 'visible' });
  await page.locator('#settings-nav-account').click();
  await page.locator('#account-meta').waitFor();
}

/** Closes Settings, so what follows is judged with it shut. */
async function closeSettings(page) {
  await page.keyboard.press('Escape');
  await page.locator('#profile-section').waitFor({ state: 'hidden' });
}

/** Waits until the account card on screen names `project`. */
const cardNames = (page, project) =>
  until(async () => {
    const name = await page.locator('#account-name').textContent();
    return name === project.label && (await page.locator('#account-meta').textContent()).includes(project.name);
  }, `the account card to name ${project.label}`);

/** A link the person cancels changes nothing. */
async function cancelledLink(s) {
  await answer(s.app, 0);
  await sendLink(s.app, (await pairingLink(KEYS.a)).link);
  await until(async () => (await asked(s.app)).length === 1, 'the pairing dialog');
  const [dialog] = await asked(s.app);
  assert.equal(dialog.message, `Connect to 127.0.0.1:${PORT}?`);
  assert.equal(dialog.defaultId, 0, 'Return cancels');
  await pause(SETTLE_MS);
  assert.equal((await config(s.page)).apiKey || '', '', 'no key was saved');
  assert.equal((await browsersOf(KEYS.a)).length, 0);
}

/** The first pairing signs the browser in and the account page says who. */
async function firstPairing(s) {
  await answer(s.app, 1);
  await sendLink(s.app, (await pairingLink(KEYS.a)).link);
  await signedInAs(s, 'a');
}

/**
 * The reported bug: pairing as someone else must show them. First with Settings shut, where
 * only the avatar and profile menu name the account, then again with the account page open.
 */
async function pairingAsSomeoneElse(s) {
  const before = (await browsersOf(KEYS.a))[0].id;
  const { code, link } = await pairingLink(KEYS.b);
  await sendLink(s.app, link);
  await signedInAs(s, 'b');
  await openAccountPage(s.page);
  await cardNames(s.page, s.projects.b);
  await sendLink(s.app, (await pairingLink(KEYS.a)).link);
  await signedInAs(s, 'a');
  await cardNames(s.page, s.projects.a);
  await sendLink(s.app, (await pairingLink(KEYS.b)).link);
  await signedInAs(s, 'b');
  await cardNames(s.page, s.projects.b);
  await closeSettings(s.page);
  assert.notEqual((await browsersOf(KEYS.b))[0].id, before, 'a new project gets a new browser id');
  return code;
}

/** A code works once: a second use says so and leaves the browser as it was. */
async function usedCode(s, code) {
  const shown = (await asked(s.app)).length;
  await sendLink(s.app, `oya://connect?code=${encodeURIComponent(code)}&server=${encodeURIComponent(WS)}`);
  await until(async () => (await asked(s.app)).length === shown + 2, 'the confirmation and then the refusal');
  const refusal = (await asked(s.app)).at(-1);
  assert.equal(refusal.message, 'Pairing failed');
  assert.match(refusal.detail, /invalid, used or expired/);
  await signedInAs(s, 'b');
}

/** Links that are not pairing links raise no dialog and change nothing. */
async function notPairingLinks(s) {
  const shown = (await asked(s.app)).length;
  const { code } = await pairingLink(KEYS.a);
  for (const link of [`oya://connect?code=${code}`, `oya://connect?code=${code}&server=ws://evil.test/ws`, 'oya://x'])
    await sendLink(s.app, link);
  await pause(SETTLE_MS);
  assert.equal((await asked(s.app)).length, shown, 'nothing was asked');
  await signedInAs(s, 'b');
}

/** Starts the app's own Sign in and returns where it sent the person, with the bridge's pending answer. */
async function startSignIn(s) {
  const from = (await opened(s.app)).length;
  const done = s.page.evaluate(() => window.oyaBrowser.startDesktopSignIn());
  await until(async () => (await opened(s.app)).length > from, 'the sign-in page to be opened');
  const login = new URL((await opened(s.app)).at(-1));
  assert.equal(login.origin + login.pathname, `${HTTP}/dashboard`);
  assert.equal(login.searchParams.get('connect'), 'desktop');
  return { done, callback: login.searchParams.get('callback'), state: login.searchParams.get('state') };
}

/** The app's Sign in: a forged return is refused, the real one switches the account with no dialog. */
async function appSignIn(s) {
  const shown = (await asked(s.app)).length;
  const { done, callback, state } = await startSignIn(s);
  const { code } = await pairingLink(KEYS.a);
  const forged = await fetch(`${callback}?code=${code}&state=${'0'.repeat(state.length)}`);
  assert.equal(forged.status, 400, 'a return with the wrong state is refused');
  assert.equal((await config(s.page)).apiKey, KEYS.b, 'and changes nothing');
  assert.equal((await fetch(`${callback}?code=${code}&state=${state}`)).status, 200);
  assert.deepEqual(await done, { ok: true });
  await signedInAs(s, 'a');
  assert.equal((await asked(s.app)).length, shown, 'the app-started sign-in asks nothing more');
}

/** A key typed into Connection settings moves the browser to that key's project. */
async function typedKey(s) {
  assert.equal(await s.page.evaluate((key) => window.oyaBrowser.saveConfig({ apiKey: key }), KEYS.b), true);
  await signedInAs(s, 'b');
  await openAccountPage(s.page);
  await cardNames(s.page, s.projects.b);
}

/** Log out forgets the key and disconnects; the remembered account signs back in. */
async function logOutAndBack(s) {
  await s.page.locator('#sign-out').click();
  await until(async () => !(await config(s.page)).apiKey, 'the key to be forgotten');
  await until(async () => (await browsersOf(KEYS.b)).length === 0, 'the server to see it leave');
  assert.equal(await account(s.page), null, 'no account is remembered after Log out');
  const saved = await s.page.evaluate(() => window.oyaBrowser.savedAccounts());
  assert.equal(saved.length, 1, 'the account is offered again');
  assert.equal(await s.page.evaluate((id) => window.oyaBrowser.useSavedAccount(id), saved[0].id), true);
  await signedInAs(s, 'b');
}

/** A restart comes back as the last account, and the file on disk never held the other one's. */
async function restart(s, userData) {
  await s.app.close();
  const saved = JSON.parse(await readFile(join(userData, 'config.json'), 'utf8'));
  assert.equal(saved.account?.project?.id ?? s.projects.b.id, s.projects.b.id, 'the saved account is the current one');
  Object.assign(s, await launchApp(userData));
  // The shell asked who it is before the names above could be added, so the avatar is judged
  // after the account page has asked again.
  await nameAccounts(s.app, s.names);
  await actsFor(s, 'b');
  await openAccountPage(s.page);
  await cardNames(s.page, s.projects.b);
  await avatarShows(s.page, s.projects.b);
}

/**
 * Launched with the server gone, the browser still says whose it is, from what it remembered:
 * the avatar shows the project's initial, not the "O" of a browser that knows no account.
 */
async function offlineLaunch(s, userData) {
  await s.app.close();
  await stopServer(s.server);
  Object.assign(s, await launchApp(userData));
  const initial = s.projects.b.name[0];
  await until(async () => (await shown(s.page)).avatar === initial, `the avatar to show ${initial} while offline`);
  assert.equal((await s.page.evaluate(() => window.oyaBrowser.getStatus())).connected, false);
  assert.equal((await account(s.page)).project.id, s.projects.b.id);
}

/** Every flow, in the order a person could meet them. */
async function scenario(s, userData) {
  s.projects = { a: await projectOf(KEYS.a), b: await projectOf(KEYS.b) };
  assert.notEqual(s.projects.a.id, s.projects.b.id, 'the two keys are two projects');
  Object.assign(s.projects.a, { label: 'Alpha' });
  Object.assign(s.projects.b, { label: 'Bravo' });
  s.names = { [s.projects.a.id]: 'Alpha', [s.projects.b.id]: 'Bravo' };
  await nameAccounts(s.app, s.names);
  const steps = { cancelledLink, firstPairing };
  for (const [name, step] of Object.entries(steps)) await run(name, () => step(s));
  const code = await run('pairingAsSomeoneElse', () => pairingAsSomeoneElse(s));
  await run('usedCode', () => usedCode(s, code));
  const rest = { notPairingLinks, appSignIn, typedKey, logOutAndBack };
  for (const [name, step] of Object.entries(rest)) await run(name, () => step(s));
  await run('restart', () => restart(s, userData));
  await run('offlineLaunch', () => offlineLaunch(s, userData));
}

/** Runs one flow, naming it in the output so a failure says which. */
async function run(name, step) {
  try {
    const result = await step();
    console.log(`  ok ${name}`);
    return result;
  } catch (error) {
    console.error(`  FAILED ${name}`);
    if (process.env.OYA_TEST_SHOTS) await s.page?.screenshot({ path: join(process.env.OYA_TEST_SHOTS, `${name}.png`) });
    throw error;
  }
}

const dataDir = await mkdtemp(join(tmpdir(), 'oya-pairing-data-'));
const userData = await mkdtemp(join(tmpdir(), 'oya-pairing-app-'));
await writeFile(join(userData, 'config.json'), JSON.stringify({ ui: { importOffered: true } }));
const s = { server: await startServer(dataDir) };
try {
  Object.assign(s, await launchApp(userData));
  await scenario(s, userData);
  console.log('PASS pairing: every flow leaves the browser acting for, and naming, the right account');
} finally {
  await s.app?.close().catch(() => {});
  await stopServer(s.server);
  await rm(dataDir, { recursive: true, force: true });
  await rm(userData, { recursive: true, force: true });
}
