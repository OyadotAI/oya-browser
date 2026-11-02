/**
 * Incognito windows in the real desktop app (stock Electron): a site signed in
 * to in an incognito window stays out of the normal window's cookie jar, out
 * of history and out of the saved session, and the window is marked as
 * incognito from its first paint. Run with `npm run test:incognito`.
 */
/* global document, window */
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createServer } from 'node:http';
import { fileURLToPath } from 'node:url';
import { developmentExecutable } from '../../src/dev/launch.ts';
import { _electron as electron } from '../../../ui/node_modules/playwright/index.mjs';

const profile = await mkdtemp(join(tmpdir(), 'oya-incognito-test-'));
await writeFile(join(profile, 'config.json'), JSON.stringify({ ui: { importOffered: true } }));

/** A site that signs in by cookie and says who it thinks you are. */
const site = createServer((req, res) => {
  res.setHeader('Content-Type', 'text/html');
  if (req.url === '/login-incognito') res.setHeader('Set-Cookie', 'who=incognito; Path=/');
  const who = /who=(\w+)/.exec(req.headers.cookie || '')?.[1] || 'nobody';
  res.end(`<!doctype html><title>Page ${req.url.slice(1)}</title><p id="who">${who}</p>`);
});

/** Waits until a page at `path` has loaded in some window, and answers its text. */
const pageText = (application, path) =>
  application.evaluate(async ({ webContents }, path) => {
    for (let attempt = 0; attempt < 100; attempt++) {
      const page = webContents.getAllWebContents().find((c) => c.getURL().endsWith(path) && !c.isLoading());
      if (page) return page.executeJavaScript('document.body.innerText');
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error('page never loaded: ' + path);
  }, path);

/** The shell page of the incognito window, once it has painted. */
async function incognitoShell(application) {
  for (let attempt = 0; attempt < 100; attempt++) {
    const shell = application.windows().find((w) => w.url().includes('incognito=true'));
    if (shell) return shell;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error('no incognito window opened');
}

let application;
try {
  await new Promise((resolve) => site.listen(0, '127.0.0.1', resolve));
  const url = `http://127.0.0.1:${site.address().port}/`;
  application = await electron.launch({
    executablePath: process.env.OYA_TEST_EXECUTABLE || developmentExecutable(),
    args: [fileURLToPath(new URL('../../', import.meta.url))],
    cwd: fileURLToPath(new URL('../../', import.meta.url)),
    env: {
      ...process.env,
      OYA_USER_DATA_DIR: profile,
      OYA_API_KEY: '',
      OYA_AUTO_CONNECT: 'false',
      OYA_SERVER_URL: '',
      OYA_REMOTE_DEBUGGING_PORT: '0',
    },
  });
  const page = await application.firstWindow();
  page.setDefaultTimeout(8000);
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.waitForFunction(() => document.documentElement.dataset.ready === 'true');
  await page.locator('#tab-key').click();
  await page.locator('#btn-manual').click();
  await page.locator('#btn-skip').click();
  await page.locator('#start-page').waitFor();
  assert.equal(await page.evaluate(() => document.documentElement.dataset.incognito), 'false');

  // Open one, as File > New Incognito Window does.
  await page.evaluate(() => window.oyaBrowser.newIncognitoWindow());
  const incognito = await incognitoShell(application);
  incognito.on('pageerror', (error) => errors.push(error.message));
  await incognito.waitForFunction(() => document.documentElement.dataset.ready === 'true');
  assert.equal(await incognito.evaluate(() => document.documentElement.dataset.incognito), 'true');
  assert.equal(await incognito.evaluate(() => document.documentElement.dataset.theme), 'dark', 'always dark');
  assert.match(await incognito.locator('#start-page').innerText(), /isn't a different identity/);
  assert.equal(await incognito.locator('#start-input').count(), 0, 'no task box for an agent');
  assert.equal(await incognito.locator('.incognito-badge').innerText(), 'Incognito');
  assert.equal(await incognito.locator('#btn-dev').count(), 0, 'no Ask button');

  // Sign in to the site in incognito; the normal window still sees nobody.
  await incognito.evaluate((u) => window.oyaBrowser.navigate(u), url + 'login-incognito');
  await pageText(application, '/login-incognito');
  await incognito.evaluate((u) => window.oyaBrowser.navigate(u), url + 'whoami-incognito');
  assert.match(await pageText(application, '/whoami-incognito'), /incognito/, 'signed in, in incognito');
  const titles = await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().map((w) => w.title));
  assert(
    titles.some((t) => t.endsWith('Oya Browser (Incognito)')),
    'the incognito window says so: ' + titles,
  );
  await page.evaluate((u) => window.oyaBrowser.navigate(u), url + 'visit-normal');
  assert.match(await pageText(application, '/visit-normal'), /nobody/, 'the incognito cookie stayed in incognito');

  // History has the normal visit, never the incognito one.
  const suggested = async (query) =>
    (await page.evaluate((q) => window.oyaBrowser.addressSuggestions(q), query)).map((s) => s.url).join(' ');
  assert.match(await suggested('visit-normal'), /visit-normal/, 'history works at all');
  assert.doesNotMatch(await suggested('incognito'), /incognito/, 'no incognito history');

  // Close the incognito window; the saved session never names its pages.
  await application.evaluate(({ BrowserWindow }) =>
    BrowserWindow.getAllWindows()
      .find((w) => w.title.includes('(Incognito)'))
      .close(),
  );
  await page.waitForTimeout(500);
  assert.equal(application.windows().filter((w) => w.url().includes('incognito=true')).length, 0);
  const saved = await readFile(join(profile, 'session-tabs.json'), 'utf8').catch(() => '');
  assert.match(saved, /visit-normal/, 'the session was saved');
  assert.doesNotMatch(saved, /incognito/, 'no incognito page in the saved session');

  // A new incognito window starts signed out: the jar went with the last window.
  await page.evaluate(() => window.oyaBrowser.newIncognitoWindow());
  const fresh = await incognitoShell(application);
  await fresh.waitForFunction(() => document.documentElement.dataset.ready === 'true');
  await fresh.evaluate((u) => window.oyaBrowser.navigate(u), url + 'check-fresh');
  assert.match(await pageText(application, '/check-fresh'), /nobody/, 'a new incognito session is empty');
  assert.deepEqual(errors, [], 'no renderer errors');
  console.log('Incognito passed: marked window, separate jar, no history, not saved, emptied on close.');
} finally {
  await application?.close();
  await new Promise((resolve) => site.close(resolve));
  await rm(profile, { recursive: true, force: true });
}
