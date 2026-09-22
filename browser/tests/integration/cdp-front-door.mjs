/**
 * End-to-end check of the CDP front door (cdp-front-door.js), driven the way a
 * benchmark harness drives it. Needs a running browser and playwright-core:
 *   docker run -d -p 9222:9222 -e OYA_REMOTE_DEBUGGING_PORT=9222 \
 *     -e OYA_USER_DATA_DIR=/data -v <userData>:/data oya-browser:bench
 *   npm i --no-save playwright-core
 *   node tests/integration/cdp-front-door.mjs http://127.0.0.1:9222 <userData>/profiles/<id>.json
 * Code passed to page.evaluate runs in the page.
 */
/* global innerWidth, innerHeight */
import { chromium } from 'playwright-core';
import assert from 'node:assert/strict';
import fs from 'node:fs';

const [base = 'http://127.0.0.1:9222', profilePath] = process.argv.slice(2);
if (!profilePath) {
  console.error('usage: node tests/integration/cdp-front-door.mjs <cdp base url> <profile json>');
  process.exit(1);
}
const profile = JSON.parse(fs.readFileSync(profilePath, 'utf8'));

const version = await (await fetch(`${base}/json/version`)).json();
assert.match(
  version.webSocketDebuggerUrl,
  new RegExp(`^ws://${new URL(base).host}/devtools/browser/`),
  'ws URL must point at the front door',
);
const list = await (await fetch(`${base}/json/list`)).json();
assert.ok(!list.some((t) => /renderer\/index\.html/.test(t.url)), 'UI must not be listed');

const t0 = Date.now();
const browser = await chromium.connectOverCDP(base);
const context = browser.contexts()[0];
assert.ok(context, 'default context');
assert.ok(!context.pages().some((p) => /renderer\/index\.html/.test(p.url())), 'UI must not be a page');

// Target.createTarget is "Not supported" in bare Electron; the front door opens a real tab.
const page = await context.newPage();
console.log(`newPage in ${Date.now() - t0}ms`);
await page.goto('https://example.com', { waitUntil: 'domcontentloaded' });
const got = await page.evaluate(() => ({
  cores: navigator.hardwareConcurrency,
  languages: navigator.languages,
  viewport: [innerWidth, innerHeight],
  tz: Intl.DateTimeFormat().resolvedOptions().timeZone,
  ua: navigator.userAgent,
  webdriver: navigator.webdriver,
}));
console.log('first page:', got);
// The first document of a tab is the one anti-bot vendors judge.
assert.equal(got.cores, profile.navigator.hardwareConcurrency, 'cores from persona');
assert.deepEqual(got.languages, profile.navigator.languages, 'languages from persona');
// On a person's own computer with no proxy the persona keeps the machine's timezone (the
// owner's decision: a spoofed zone over a home IP is itself a signal); the Docker image uses the persona's.
const machineTz = Intl.DateTimeFormat().resolvedOptions().timeZone;
assert.ok([profile.timezone, machineTz].includes(got.tz), `timezone from persona or machine (got ${got.tz})`);
assert.ok(got.viewport[0] > 0 && got.viewport[1] > 0, 'viewport laid out');
assert.ok(!/Electron|oya-browser|HeadlessChrome/.test(got.ua), 'clean UA');
assert.equal(got.webdriver, false, 'no webdriver flag');

// A CDP client opens web addresses only, as the address bar does: this is the person's machine.
await assert.rejects(page.goto('file:///etc/hosts'), /Only http and https addresses/, 'file: navigation refused');
const refusedNew = await fetch(`${base}/json/new?file:///etc/hosts`, { method: 'PUT' });
assert.equal(refusedNew.status, 400, '/json/new refuses a file: url');
const raw = await context.newCDPSession(page);
await assert.rejects(
  raw.send('Page.navigate', { url: 'file:///etc/hosts' }),
  /Only http and https addresses/,
  'raw Page.navigate to file: refused',
);
await assert.rejects(
  raw.send('Target.exposeDevToolsProtocol', { targetId: 'x', bindingName: 'cdp' }),
  /around the front door/,
  'a page binding around the door refused',
);
assert.ok(!/etc\/hosts/.test(page.url()), 'the page never left the web');

const second = await context.newPage();
await second.close();
assert.ok(!context.pages().includes(second), 'closeTarget works');
await browser.close().catch(() => {});
console.log('ok, front door hides the UI, opens protected tabs, closes them, refuses local files');
