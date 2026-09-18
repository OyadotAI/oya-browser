#!/usr/bin/env node
/**
 * Native JavaScript dialogs against a real Chrome.
 *
 * With the Page domain enabled and nobody answering
 * Page.javascriptDialogOpening, Chromium blocks the renderer until the client
 * answers — so an alert() on a login page wedged the tab and every later command
 * ate its whole timeout with nothing to show for it. This proves the three
 * things that fixes it: an alert is answered on its own and its text survives,
 * a confirm is held and reported instead of hanging, and handle_dialog
 * unblocks the page.
 *
 * Skipped when no Chrome binary is present.
 */

import { createServer } from 'http';
import { spawn } from 'child_process';
import { once } from 'events';
import { mkdtempSync, existsSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { CDPDriver } from './src/drivers/cdp.js';
import { AUTO_ACCEPT, describe } from './src/dialogs.js';
import { removeScratch } from './test-support/scratch.js';

let passed = 0, failed = 0;
const assert = (c, label) => {
  if (c) { console.log(`  ✅ ${label}`); passed++; }
  else { console.log(`  ❌ ${label}`); failed++; }
};
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

console.log('\n1️⃣  The policy itself...');
assert(AUTO_ACCEPT.has('alert'), 'an alert is answered for the agent — it has one button');
assert(AUTO_ACCEPT.has('beforeunload'), 'beforeunload is accepted — the agent meant to navigate');
assert(!AUTO_ACCEPT.has('confirm') && !AUTO_ACCEPT.has('prompt'),
  'a confirm or prompt is a decision, so it is never answered automatically');
assert(describe({ type: 'alert', message: 'Invalid login' }, true).includes('Invalid login'),
  'the reported note carries the real message, not a fixed string');
assert(/handle_dialog/.test(describe({ type: 'confirm', message: 'Delete?' })),
  'a held dialog tells the caller how to answer it');
assert(describe({ type: 'prompt', message: 'Name?', defaultPrompt: 'Ada' }).includes('Ada'),
  'a prompt reports its default value');

const CHROME = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome', '/usr/bin/chromium', '/usr/bin/chromium-browser',
].find((p) => existsSync(p));

if (!CHROME) {
  console.log('\n⏭  No Chrome binary found — skipping the live dialog test');
  console.log(`\n  ${passed} passed, ${failed} failed`);
  process.exit(failed ? 1 : 0);
}

// The shape of the reported bug: a button whose handler alerts, like the login
// on radmd.com. Plus a confirm, which must not be answered for the agent.
const PAGE = `<!doctype html><title>dialog fixture</title>
<button id="warn" onclick="alert('Invalid login'); document.title='alerted'">Login</button>
<button id="ask" onclick="document.title = confirm('Delete this?') ? 'confirmed' : 'cancelled'">Delete</button>`;

const site = createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end(PAGE);
});
await new Promise((r) => site.listen(0, '127.0.0.1', r));
const siteUrl = `http://127.0.0.1:${site.address().port}/`;

const profile = mkdtempSync(join(tmpdir(), 'oya-dialog-'));
const chrome = spawn(CHROME, [
  '--headless=new', '--remote-debugging-port=0', '--no-first-run', '--no-default-browser-check',
  `--user-data-dir=${profile}`, 'about:blank',
], { stdio: ['ignore', 'ignore', 'pipe'] });

const wsUrl = await new Promise((resolve, reject) => {
  let buf = '';
  const timer = setTimeout(() => reject(new Error('Chrome did not report a DevTools endpoint')), 20000);
  chrome.stderr.on('data', (d) => {
    buf += d.toString();
    const m = buf.match(/ws:\/\/127\.0\.0\.1:\d+\/devtools\/browser\/[a-f0-9-]+/);
    if (m) { clearTimeout(timer); resolve(m[0]); }
  });
  chrome.on('exit', () => { clearTimeout(timer); reject(new Error('Chrome exited early')); });
});

let driver;
try {
  driver = await new CDPDriver({ wsUrl, provider: 'chrome' }).connect();
  await driver.send('navigate', { url: siteUrl });

  console.log('\n2️⃣  An alert is answered and its text comes back...');
  const analyzed = await driver.send('analyze', {});
  const login = (analyzed.data?.elements || []).find((e) => e.text === 'Login');
  assert(!!login, 'the fixture analyzed');

  // The bug was here: this call used to never return.
  const clicked = await Promise.race([
    driver.send('click', { element_id: login.id }),
    wait(15000).then(() => ({ ok: false, error: 'HUNG' })),
  ]);
  assert(clicked.error !== 'HUNG', 'clicking a button that alerts returns instead of wedging the tab');
  assert(String(clicked.data?.dialog || '').includes('Invalid login'),
    `the alert's text is reported on the result (got: ${JSON.stringify(clicked.data?.dialog) || 'nothing'})`);
  await wait(200);
  assert(await driver.evaluate('document.title') === 'alerted', 'the page carried on past the alert');
  assert(!(await driver.send('analyze', {})).data?.dialog, 'the note is reported once, not on every result');

  console.log('\n3️⃣  A confirm is held, not guessed...');
  const els = (await driver.send('analyze', {})).data.elements;
  const del = els.find((e) => e.text === 'Delete');
  const onConfirm = await Promise.race([
    driver.send('click', { element_id: del.id }),
    wait(15000).then(() => ({ ok: false, error: 'HUNG' })),
  ]);
  assert(onConfirm.error !== 'HUNG', 'clicking a button that confirms returns');
  // Nothing may read the page here: the renderer is blocked, which is the point.
  // That the dialog is still open below is the proof nobody answered it.
  assert(/Delete this\?/.test(onConfirm.error || ''), 'the click reports the question instead of guessing an answer');

  // The page is blocked now. Every command must say so immediately rather than
  // burning its timeout, which is what turned one dialog into a dead run.
  const started = Date.now();
  const blocked = await driver.send('analyze', {});
  assert(blocked.ok === false && /confirm dialog is open/.test(blocked.error),
    `a command against a blocked page fails with the dialog's text (got: ${blocked.error})`);
  assert(blocked.error.includes('Delete this?'), 'the error carries the message the page asked');
  assert(Date.now() - started < 2000, 'and it fails fast instead of waiting out the timeout');

  console.log('\n4️⃣  handle_dialog unblocks it...');
  const answered = await driver.send('handle_dialog', { accept: true });
  assert(answered.ok && answered.data.accepted === true, 'handle_dialog answers the held dialog');
  await wait(200);
  assert(await driver.evaluate('document.title') === 'confirmed', 'the page saw the answer');
  assert((await driver.send('analyze', {})).ok, 'commands work again once it is answered');
  assert((await driver.send('handle_dialog', { accept: true })).ok === false,
    'answering when nothing is open is an error, not a hang');
} catch (e) {
  console.log(`  ❌ threw: ${e.message}`);
  failed++;
} finally {
  driver?.close();
  const exited = once(chrome, 'exit');
  chrome.kill('SIGTERM');
  await exited;
  await new Promise((r) => site.close(r));
  removeScratch(profile);
}

console.log('\n──────────────────────────────────────────────────');
console.log(`  ${passed} passed, ${failed} failed`);
console.log('──────────────────────────────────────────────────');
process.exit(failed ? 1 : 0);
