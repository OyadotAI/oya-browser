#!/usr/bin/env node
/**
 * CDP driver against a real Chrome.
 *
 * Verifies that a browser the control plane dials out to answers the same
 * action vocabulary as the Oya client that dials in — including analyze and
 * click-by-element_id, which depend on the injected analyzer.
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
import { getFingerprintForPersona } from './src/fingerprint.js';
import { removeScratch } from './test-support/scratch.js';

const CHROME = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome', '/usr/bin/chromium', '/usr/bin/chromium-browser',
].find((p) => existsSync(p));

if (!CHROME) {
  console.log('⏭  No Chrome binary found — skipping CDP driver test');
  process.exit(0);
}

let passed = 0, failed = 0;
const assert = (c, label) => {
  if (c) { console.log(`  ✅ ${label}`); passed++; }
  else { console.log(`  ❌ ${label}`); failed++; }
};
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

const PAGE = `<!doctype html><title>CDP fixture</title>
<button onclick="document.title='clicked'">Press me</button>
<input placeholder="name">
<div style="height:3000px"></div>`;

const site = createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end(PAGE);
});
await new Promise((r) => site.listen(0, '127.0.0.1', r));
const siteUrl = `http://127.0.0.1:${site.address().port}/`;

const profile = mkdtempSync(join(tmpdir(), 'oya-cdp-'));
const chrome = spawn(CHROME, [
  '--headless=new', '--remote-debugging-port=0', '--no-first-run', '--no-default-browser-check',
  `--user-data-dir=${profile}`, 'about:blank',
], { stdio: ['ignore', 'ignore', 'pipe'] });

// Chrome prints the DevTools endpoint on stderr when the port is 0.
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
  console.log('\n1️⃣  Connect and drive a real Chrome over CDP...');
  driver = await new CDPDriver({ wsUrl, provider: 'chrome' }).connect();
  assert(driver.isAlive(), 'Driver connected and attached to a page target');

  const nav = await driver.send('navigate', { url: siteUrl });
  assert(nav.ok, 'navigate succeeds');
  assert(nav.data.title === 'CDP fixture', `navigate returns the page title (got "${nav.data.title}")`);

  const read = await driver.send('read_page');
  assert(read.data.url.startsWith('http://127.0.0.1'), 'read_page returns the current URL');

  const shot = await driver.send('screenshot');
  assert(/^data:image\/jpeg;base64,/.test(shot.data.screenshot), 'screenshot returns a JPEG data URL');
  assert(shot.data.screenshot.length > 1000, 'screenshot is non-trivial in size');

  console.log('\n2️⃣  Analyzer parity with the Oya client...');
  const analyzed = await driver.send('analyze', {});
  assert(analyzed?.ok === true, 'analyze succeeds against a CDP browser');
  const elements = analyzed.data?.elements || [];
  assert(elements.length >= 2, `the injected analyzer indexed the page (${elements.length} elements)`);

  // Same element shape the Oya client produces, so an agent cannot tell the
  // two client types apart.
  const button = elements.find((e) => e.tag === 'button');
  const input = elements.find((e) => e.tag === 'input');
  assert(button?.text === 'Press me', 'analyzer returns the button with its text');
  assert(typeof button?.id === 'number', 'elements carry the numeric ids click expects');

  console.log('\n3️⃣  Real input reaches the real page...');
  const clicked = await driver.send('click', { element_id: button.id });
  assert(clicked.ok, 'click by analyzer element id resolves and dispatches');
  await wait(200);
  assert((await driver.send('read_page')).data.title === 'clicked', 'the click actually fired the page handler');

  await driver.send('type', { element_id: input.id, text: 'hello fleet' });
  // Query by a page-authored attribute: the analyzer's own tag is randomised
  // per session now, so it is deliberately not something a caller can rely on.
  const typed = await driver.evaluate(`document.querySelector('input[placeholder="name"]').value`);
  assert(typed === 'hello fleet', `type lands in the real input (got "${typed}")`);

  const before = await driver.evaluate('window.scrollY');
  await driver.send('scroll-down', {});
  await wait(300);
  assert(await driver.evaluate('window.scrollY') > before, 'scroll-down moves the page');

  await driver.evaluateMain(`window.scrollTo(0, 0); const pane = document.createElement('div'); pane.id = 'scroll-pane'; pane.style = 'position:fixed;left:100px;top:100px;width:200px;height:200px;overflow:auto'; pane.innerHTML = '<div style="height:2000px">Scrollable pane</div>'; document.body.append(pane);`);
  await driver.send('scroll', { direction: 'down', amount: 120, x: 150, y: 150, smooth: false });
  await wait(300);
  assert(await driver.evaluateMain('document.getElementById("scroll-pane").scrollTop') > 0, 'live scroll targets the pane under the pointer');
  assert(await driver.evaluateMain('window.scrollY') === 0, 'scrolling a pane leaves the outer page in place');
  await driver.evaluateMain('document.getElementById("scroll-pane").remove()');

  console.log('\n4️⃣  Tab management...');
  const tabs = await driver.send('list-tabs');
  assert(Array.isArray(tabs.data.tabs) && tabs.data.tabs.length >= 1, 'list-tabs enumerates page targets');
  const opened = await driver.send('new-tab', { url: siteUrl });
  assert(opened.ok && opened.data.id, 'new-tab creates and attaches to a target');
  assert((await driver.send('list-tabs')).data.tabs.length >= 2, 'the new tab is listed');
  assert((await driver.send('close-tab', { id: opened.data.id })).ok, 'close-tab closes it');

  console.log('\n5️⃣  Unsupported actions fail cleanly...');
  const bogus = await driver.send('teleport', {});
  assert(bogus.ok === false && /Unsupported action/.test(bogus.error), 'an unknown action returns a clear error, not a throw');

  console.log('\n6️⃣  element_id cannot smuggle code into the page...');
  // Driving a browser means evaluating source in it; a caller-supplied id must
  // never reach that source. Analyzer ids are integers, so anything else is out.
  await driver.send('navigate', { url: siteUrl });
  const hostile = [
    `1"]); window.__pwned = 1; //`,
    `1'); window.__pwned = 1; //`,
    '1]); window.__pwned = 1; //',
    '__proto__',
    { toString() { return '1'; } },
  ];
  let rejected = 0;
  for (const id of hostile) {
    for (const action of ['click', 'select', 'hover', 'type']) {
      try { await driver.send(action, { element_id: id, value: 'x', text: 'x' }); }
      catch (e) { if (e.status === 400) rejected++; }
    }
  }
  assert(rejected === hostile.length * 4, `every hostile element_id is rejected (${rejected}/${hostile.length * 4})`);
  assert(await driver.evaluate('window.__pwned === undefined'), 'nothing was injected into the page');

  const evalAttempt = await driver.send('evaluate', { expression: 'window.__pwned = 1' });
  assert(evalAttempt.ok === false, 'arbitrary evaluate is not exposed as a command');
  assert(await driver.evaluate('window.__pwned === undefined'), 'the evaluate attempt changed nothing');

  console.log('\n7\ufe0f\u20e3b  Pointer and keyboard parity with the Oya client...');
  {
    // The dashboard drives a browser with the Oya client's vocabulary and
    // must never have to ask which kind it is talking to.
    await driver.send('navigate', { url: siteUrl });
    const cc = await driver.send('click_coordinates', { x: 5, y: 5 });
    assert(cc.ok, 'click_coordinates is accepted (aliased to click-coords)');
    const mm = await driver.send('mouse_move', { x: 10, y: 10 });
    assert(mm.ok, 'mouse_move is accepted');
    // Focus the input by clicking it, then type like a person.
    const a = await driver.send('analyze');
    const input = (a.data?.elements || []).find((e) => e.type === 'input');
    if (input) {
      await driver.send('click', { element_id: input.id });
      const kt = await driver.send('keyboard_type', { text: 'hello' });
      assert(kt.ok, 'keyboard_type is accepted');
      const pk = await driver.send('press_key', { key: '!' });
      assert(pk.ok, 'press_key with a printable character is accepted');
      const val = await driver.evaluateMain(`document.querySelector('input')?.value`);
      assert(val === 'hello!', `typed text landed in the focused input (got "${val}")`);
    }
    const dc = await driver.send('double_click', { x: 5, y: 5 });
    assert(dc.ok, 'double_click is accepted');
    const dr = await driver.send('drag', { from_x: 5, from_y: 5, to_x: 50, to_y: 50 });
    assert(dr.ok, 'drag is accepted');
  }

  console.log('\n\u0038\ufe0f\u20e3  The page carries no trace of this product...');
  {
    // `typeof window.analyzePage === 'function'` is a one-line, 100%-precision
    // detector. It has to be checked in the page's own world — driver.evaluate
    // runs in the isolated one, where of course the analyzer is present.
    const inPage = (expr) => driver.evaluateMain(expr);
    assert(await inPage(`typeof window.analyzePage === 'undefined'`), 'analyzePage is not a page global');
    assert(await inPage(
      `['__acAnalyzerLoaded','__acFindElement','__acQueryShadow','__oyaInternalCall']
         .every((k) => typeof window[k] === 'undefined')`), 'no __ac* globals leak into the page');
    assert(await inPage(`document.querySelectorAll('[data-ac-id]').length === 0`),
      'the analyzer does not brand the DOM with a constant attribute');
    // The analyzer still has to work from where it lives.
    assert((await driver.send('analyze')).ok, 'analyze still works from the isolated world');
  }

  console.log('\nRecording survives document replacement without a status poll...');
  await driver.send('navigate', { url: siteUrl });
  // Desktop deliberately does not enable Runtime: binding events must still work.
  await driver.conn.send('Runtime.disable', {}, driver.sessionId);
  const startRecording = await driver.send('record', { mode: 'start' });
  assert(startRecording.ok, 'recording starts');
  await driver.evaluateMain(`document.querySelector('input').value = 'prefilled';
    document.querySelector('input').focus();
    document.querySelector('input').value = '';
    document.querySelector('input').dispatchEvent(new Event('input', { bubbles: true }));
    document.querySelector('input').dispatchEvent(new Event('change', { bubbles: true }));
    const a = document.createElement('a'); a.href = '/next'; a.id = 'next-link'; a.textContent = 'Next';
    document.body.appendChild(a); a.click();`);
  await wait(300);
  // Use the page's world here: no analyzer/read/status command may reinject it.
  await driver.evaluateMain(`document.querySelector('input').focus();
    document.querySelector('input').value = 'second page';
    document.querySelector('input').dispatchEvent(new Event('input', { bubbles: true }));`);
  const recordedFlow = await driver.send('record', { mode: 'stop' });
  assert(recordedFlow.ok, 'recording stops');
  assert(recordedFlow.data.steps.some(s => s.action === 'click' && s.el.domId === 'next-link'), 'navigation click survives without polling');
  assert(recordedFlow.data.steps.some(s => s.action === 'type' && s.text === ''), 'clearing a field is retained');
  assert(recordedFlow.data.steps.some(s => s.action === 'type' && s.text === 'second page'), 'new document records before first poll and stop flushes current field');
  assert(await driver.evaluateMain(`typeof window.__acRecordSink === 'undefined'`), 'recorder binding stays out of site globals');
  await driver.send('record', { mode: 'start' });
  const freshFlow = await driver.send('record', { mode: 'stop' });
  assert(freshFlow.data.steps.length === 1, 'a fresh recording contains no previous steps');
  await driver.send('record', { mode: 'start' });
  await driver.evaluateMain(`const p = document.createElement('input');
    p.type = 'password'; p.name = 'secret'; document.body.appendChild(p); p.focus();
    p.value = 'never-export-this'; p.dispatchEvent(new Event('input', { bubbles: true }));`);
  const secretFlow = await driver.send('record', { mode: 'stop' });
  assert(!JSON.stringify(secretFlow).includes('never-export-this'), 'password never leaves the isolated recorder');
  assert(secretFlow.data.steps.some(s => s.text === '{{secret}}') && secretFlow.data.secrets.includes('secret'), 'password placeholder and secret name survive push capture');

  console.log('\n\u0039\ufe0f\u20e3  A persona actually reaches the page...');
  {
    const fp = getFingerprintForPersona({ id: 'cdp-test-persona', seed: 4242 });
    const d2 = await new CDPDriver({ wsUrl, provider: 'cdp', fingerprint: fp }).connect();
    try {
      await d2.send('navigate', { url: siteUrl });
      const inPage = (expr) => d2.evaluateMain(expr);
      assert(await inPage('navigator.platform') === fp.navigator.platform,
        `navigator.platform is the persona's (${fp.navigator.platform})`);
      assert(await inPage('navigator.hardwareConcurrency') === fp.navigator.hardwareConcurrency,
        "hardwareConcurrency is the persona's, not this machine's");
      assert(await inPage('Intl.DateTimeFormat().resolvedOptions().timeZone') === fp.timezone,
        `the timezone is the persona's (${fp.timezone})`);
      assert(await inPage('navigator.webdriver') === false, 'navigator.webdriver reads false');
      assert(!/HeadlessChrome/.test(await inPage('navigator.userAgent')),
        'the UA carries no headless marker');
      // Overriding the UA without metadata blanks client hints, which no real
      // browser does — the mitigation would plant the flag it was hiding.
      assert((await inPage('navigator.userAgentData.brands.length')) > 0,
        'client hints survive the user agent override');
      assert(!!(await inPage('navigator.userAgentData.platform')),
        'userAgentData.platform is populated');

      // A provider that ships its own stealth must not be double-patched:
      // their patches plus ours contradict each other, and a contradiction is
      // a stronger signal than either alone. Asserted on the driver rather
      // than the page, because both drivers attach to the same page target
      // here and would otherwise read each other's work.
      const d3 = await new CDPDriver({ wsUrl, provider: 'browserbase', fingerprint: fp }).connect();
      try {
        await d3.send('navigate', { url: siteUrl });
        assert(d3.userAgent === undefined,
          'a provider that ships its own stealth gets no user agent override');
        assert(await d3.evaluateMain(`typeof window.analyzePage === 'undefined'`),
          'and it still gets no product globals in the page');
      } finally { d3.close(); }
    } finally { d2.close(); }
  }
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
