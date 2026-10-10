#!/usr/bin/env node
/**
 * External CDP driver behavior against Oya's native adapter. Recording and
 * persona assertions use the browser-owned native lifecycle, not internal CDP.
 * The generic external-provider recording/emulation paths retain unit coverage;
 * this suite does not claim that Oya implements those protocol capabilities.
 */

import { createServer } from 'http';
import { CDPDriver, CDPConnection } from '../../src/drivers/cdp.ts';
import { getFingerprintForPersona } from '../../src/modules/personas/fingerprint.ts';

import { openNativeFixture } from '../support/native-browser.mjs';

let passed = 0,
  failed = 0;
const assert = (c, label) => {
  if (c) {
    console.log(`  ✅ ${label}`);
    passed++;
  } else {
    console.log(`  ❌ ${label}`);
    failed++;
  }
};
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

const PAGE = `<!doctype html><title>CDP fixture</title>
<button onclick="document.title='clicked'">Press me</button>
<input placeholder="name">
<div style="height:3000px"></div>`;

// What the analyzer's markdown has to carry for the agent to read a modern page:
// React-style markup with no whitespace between elements, a link wrapping a
// whole product card, and widgets whose state lives only in ARIA attributes.
const READER =
  `<!doctype html><title>Reader fixture</title><main><div>$19.99</div><div>4.5 stars</div>` +
  `<a href="/p/1"><div>Espresso machine with a long name that runs past the label cap of the analyzer</div><div>Price $249.00</div></a>` +
  `<div role="switch" aria-checked="true" tabindex="0">Dark mode</div>` +
  `<div role="tablist"><div role="tab" aria-selected="true">Overview</div><div role="tab" aria-selected="false">Specs</div></div>` +
  `<button aria-expanded="false">Menu</button><button aria-disabled="true">Buy</button></main>`;

// A mail-style app: a small chat dialog that must not hide the page, a list
// that scrolls inside a panel, a cookie banner over a button, a label that
// wraps its checkbox, an image with no alt, and a cell holding a pipe.
const APP =
  `<!doctype html><title>App fixture</title><body style="margin:0">` +
  `<div role="dialog" style="position:fixed;right:0;bottom:0;width:200px;height:120px">Chat with us</div>` +
  `<div style="height:300px;overflow-y:auto"><div style="height:2000px">Inbox</div></div>` +
  `<label><input type="checkbox"> Remember me</label><img src="/x.png">` +
  `<table><tr><td>a|b</td><td>c</td></tr></table>` +
  `<button style="position:absolute;top:250px;left:10px">Under</button>` +
  `<div style="position:fixed;top:200px;left:0;width:100%;height:150px;background:#fff">Cookie banner</div></body>`;

// Lists and tables of links (a message list, a file table): each link tagged once,
// hidden columns left out, quotes in labels escaped, and no stray whitespace.
const LISTS =
  `<!doctype html><title>Lists fixture</title><body>` +
  `<ul><li><a href="/a">Alpha</a></li><li><a href="/b">Beta</a></li></ul>` +
  `<table><tr><th style="display:none">Name</th><th>Name</th><th>Message</th></tr>` +
  `<tr><td style="display:none"><a href="/f">file</a></td><td><a href="/f">file</a></td><td><a href="/c">Revert "fix"</a></td></tr></table>` +
  `<p>Read the   <a href="/terms">terms</a> , then   continue.</p><div>   </div><div> </div></body>`;

/** Each fixture page by path; anything else is the main fixture. */
const PAGES = { '/reader': READER, '/app': APP, '/lists': LISTS };

const site = createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end(Object.hasOwn(PAGES, req.url) ? PAGES[req.url] : PAGE);
});
await new Promise((r) => site.listen(0, '127.0.0.1', r));
const siteUrl = `http://127.0.0.1:${site.address().port}/`;

const native = await openNativeFixture();
/** Connect through the authenticated public adapter, then use the driver's actual attachment lifecycle. */
async function connect(endpoint, options = {}) {
  const result = new CDPDriver({ wsUrl: endpoint.url, ...options });
  result.conn = await new CDPConnection(endpoint.url, { bearerToken: endpoint.token }).connect();
  try {
    const { targetInfos } = await result.conn.send('Target.getTargets');
    await result.attach(targetInfos[0].targetId);
    return result;
  } catch (error) {
    result.close();
    throw error;
  }
}

let driver;
try {
  console.log('\n1️⃣  Connect and drive Oya through the native external adapter...');
  await native.preparePersona({
    fingerprint: getFingerprintForPersona({ id: 'native-driver', seed: 4242 }),
    origins: {},
    cookies: [],
    now: Date.now(),
  });
  await native.send('navigate', { url: siteUrl });
  driver = await connect(await native.frontDoor());
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
  assert((await driver.evaluate('window.scrollY')) > before, 'scroll-down moves the page');

  await driver.evaluateMain(
    `window.scrollTo(0, 0); const pane = document.createElement('div'); pane.id = 'scroll-pane'; pane.style = 'position:fixed;left:100px;top:100px;width:200px;height:200px;overflow:auto'; pane.innerHTML = '<div style="height:2000px">Scrollable pane</div>'; document.body.append(pane);`,
  );
  await driver.send('scroll', { direction: 'down', amount: 120, x: 150, y: 150, smooth: false });
  await wait(300);
  assert(
    (await driver.evaluateMain('document.getElementById("scroll-pane").scrollTop')) > 0,
    'live scroll targets the pane under the pointer',
  );
  assert((await driver.evaluateMain('window.scrollY')) === 0, 'scrolling a pane leaves the outer page in place');
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
  assert(
    bogus.ok === false && /Unsupported action/.test(bogus.error),
    'an unknown action returns a clear error, not a throw',
  );

  console.log('\n6️⃣  element_id cannot smuggle code into the page...');
  // Driving a browser means evaluating source in it; a caller-supplied id must
  // never reach that source. Analyzer ids are integers, so anything else is out.
  await driver.send('navigate', { url: siteUrl });
  const hostile = [
    `1"]); window.__pwned = 1; //`,
    `1'); window.__pwned = 1; //`,
    '1]); window.__pwned = 1; //',
    '__proto__',
    {
      toString() {
        return '1';
      },
    },
  ];
  let rejected = 0;
  for (const id of hostile) {
    for (const action of ['click', 'select', 'hover', 'type']) {
      try {
        await driver.send(action, { element_id: id, value: 'x', text: 'x' });
      } catch (e) {
        if (e.status === 400) rejected++;
      }
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

  console.log('\n7\ufe0f\u20e3c  The markdown carries what the agent needs to read the page...');
  {
    await driver.send('navigate', { url: siteUrl + 'reader' });
    const md = (await driver.send('analyze')).data?.markdown || '';
    assert(/\$19\.99\s+4\.5 stars/.test(md), 'adjacent blocks are separated, not run together');
    assert(md.includes('Price $249.00'), 'a link wrapping a card keeps the text its label cannot hold');
    assert(/"Dark mode" \(checked\)/.test(md), 'an aria-checked switch reads as checked');
    assert(/"Overview" \(selected\)/.test(md) && !/"Specs" \(selected\)/.test(md), 'the selected tab is marked');
    assert(/button "Menu" \(collapsed\)/.test(md), 'a collapsed menu button says so');
    assert(/button "Buy" \(disabled\)/.test(md), 'an aria-disabled button reads as disabled');

    // The same page as TOON: one table of blocks, the switch a row with its id and state.
    const toon = (await driver.send('analyze', { format: 'toon' })).data;
    assert(toon.format === 'toon' && toon.markdown === undefined, 'a TOON analysis says so and carries no markdown');
    assert(/^blocks\[\d+\]\{id,region,kind,text,target,state\}:$/m.test(toon.page), 'a TOON page is one block table');
    assert(
      /^ {2}\d+,main,checkbox,Dark mode,"",checked$/m.test(toon.page),
      'the switch is a row with its id and state',
    );

    await driver.send('navigate', { url: siteUrl + 'app' });
    const app = (await driver.send('analyze')).data;
    assert(
      !/^modal:/m.test(app.markdown) && app.markdown.includes('Inbox'),
      'a small non-modal dialog does not hide the page',
    );
    assert(/^panelScroll: 0% /m.test(app.markdown), 'a panel that scrolls its own content is reported');
    const under = app.elements.find((e) => e.text === 'Under');
    assert(
      under?.covered === true && /^covered: 2 /m.test(app.markdown),
      'a button (and the checkbox) behind a banner are marked covered',
    );
    assert(app.markdown.split('Remember me').length === 2, 'a label wrapping its checkbox is not repeated');
    assert(!app.markdown.includes('[image]'), 'an image with no alt adds nothing');
    assert(app.markdown.includes('a\\|b'), 'a pipe inside a table cell is escaped');

    await driver.send('navigate', { url: siteUrl + 'lists' });
    const lists = (await driver.send('analyze')).data;
    const ids = [...lists.markdown.matchAll(/\[#(\d+) /g)].map((m) => Number(m[1]));
    assert(
      ids.join() === ids.map((_, i) => i + 1).join() && lists.elements.length === ids.length,
      `links in lists and tables are tagged once, with no phantom ids (${ids.join()})`,
    );
    const headers = lists.elements.filter((element) => element.tag === 'th').map((element) => element.text);
    assert(headers.join('|') === 'Name|Message', 'a hidden table column is left out');
    assert(lists.markdown.includes('"Revert \\"fix\\""'), 'quotes inside a label are escaped');
    const body = lists.markdown.split('---\n').pop();
    assert(!/ {2,}|^ +$|\] ,/m.test(body), 'no double spaces, whitespace-only lines or space before punctuation');
  }

  console.log('\n\u0038\ufe0f\u20e3  The page carries no trace of this product...');
  {
    // `typeof window.analyzePage === 'function'` is a one-line, 100%-precision
    // detector. It has to be checked in the page's own world, driver.evaluate
    // runs in the isolated one, where of course the analyzer is present.
    const inPage = (expr) => driver.evaluateMain(expr);
    assert(await inPage(`typeof window.analyzePage === 'undefined'`), 'analyzePage is not a page global');
    assert(
      await inPage(
        `['__acAnalyzerLoaded','__acFindElement','__acQueryShadow','__oyaInternalCall']
         .every((k) => typeof window[k] === 'undefined')`,
      ),
      'no __ac* globals leak into the page',
    );
    assert(
      await inPage(`document.querySelectorAll('[data-ac-id]').length === 0`),
      'the analyzer does not brand the DOM with a constant attribute',
    );
    // The analyzer still has to work from where it lives.
    assert((await driver.send('analyze')).ok, 'analyze still works from the isolated world');
  }

  console.log('\nRecording survives document replacement without a status poll...');
  await driver.send('navigate', { url: siteUrl });
  // Native recording must survive without enabling the external Runtime event domain.
  await driver.conn.send('Runtime.disable', {}, driver.sessionId);
  const startRecording = await native.send('record', { mode: 'start' });
  assert(startRecording.ok, 'recording starts');
  await driver.evaluateMain(`document.querySelector('input').value = 'prefilled';
    document.querySelector('input').focus();
    document.querySelector('input').select();
    const a = document.createElement('a'); a.href = '/next'; a.id = 'next-link'; a.textContent = 'Next';
    document.body.prepend(a);`);
  await driver.send('press-key', { key: 'Backspace' });
  const linkPoint = await driver.evaluateMain(
    `(() => { const r = document.getElementById('next-link').getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`,
  );
  await driver.send('click-coords', linkPoint);
  await wait(300);
  // Use the page's world here: no analyzer/read/status command may reinject it.
  await driver.evaluateMain(`document.querySelector('input').focus()`);
  await driver.send('keyboard_type', { text: 'second page' });
  const recordedFlow = await native.send('record', { mode: 'stop' });
  assert(recordedFlow.ok, 'recording stops');
  assert(
    recordedFlow.data.steps.some((s) => s.action === 'click' && s.el.domId === 'next-link'),
    'navigation click survives without polling',
  );
  assert(
    recordedFlow.data.steps.some((s) => s.action === 'type' && s.text === ''),
    'clearing a field is retained',
  );
  assert(
    recordedFlow.data.steps.some((s) => s.action === 'type' && s.text === 'second page'),
    'new document records before first poll and stop flushes current field',
  );
  assert(
    await driver.evaluateMain(`typeof window.__acRecordSink === 'undefined'`),
    'recorder binding stays out of site globals',
  );
  await native.send('record', { mode: 'start' });
  const freshFlow = await native.send('record', { mode: 'stop' });
  assert(freshFlow.data.steps.length === 1, 'a fresh recording contains no previous steps');
  await native.send('record', { mode: 'start' });
  await driver.evaluateMain(`const p = document.createElement('input');
    p.type = 'password'; p.name = 'secret'; document.body.appendChild(p); p.focus();`);
  await driver.send('keyboard_type', { text: 'never-export-this' });
  const secretFlow = await native.send('record', { mode: 'stop' });
  assert(!JSON.stringify(secretFlow).includes('never-export-this'), 'password never leaves the isolated recorder');
  assert(
    secretFlow.data.steps.some((s) => s.text === '{{secret}}') && secretFlow.data.secrets.includes('secret'),
    'password placeholder and secret name survive push capture',
  );

  console.log('\n\u0039\ufe0f\u20e3  A persona actually reaches the page...');
  {
    const fp = getFingerprintForPersona({ id: 'cdp-test-persona', seed: 4242 });
    const personaBrowser = await openNativeFixture();
    let d2;
    try {
      await personaBrowser.preparePersona({ fingerprint: fp, origins: {}, cookies: [], now: Date.now() });
      await personaBrowser.send('navigate', { url: siteUrl });
      const endpoint = await personaBrowser.frontDoor();
      d2 = await connect(endpoint);
      await d2.send('navigate', { url: siteUrl });
      const inPage = (expr) => d2.evaluateMain(expr);
      assert(
        (await inPage('navigator.platform')) === fp.navigator.platform,
        `navigator.platform is the persona's (${fp.navigator.platform})`,
      );
      assert(
        (await inPage('navigator.hardwareConcurrency')) === fp.navigator.hardwareConcurrency,
        "hardwareConcurrency is the persona's, not this machine's",
      );
      assert(
        (await inPage('Intl.DateTimeFormat().resolvedOptions().timeZone')) ===
          Intl.DateTimeFormat().resolvedOptions().timeZone,
        'a direct native session uses the host timezone, matching its unproxied egress policy',
      );
      assert((await inPage('navigator.webdriver')) === false, 'navigator.webdriver reads false');
      assert(!/HeadlessChrome/.test(await inPage('navigator.userAgent')), 'the UA carries no headless marker');
      // Overriding the UA without metadata blanks client hints, which no real
      // browser does, the mitigation would plant the flag it was hiding.
      assert(
        (await inPage('navigator.userAgentData.brands.length')) > 0,
        'client hints survive the user agent override',
      );
      assert(!!(await inPage('navigator.userAgentData.platform')), 'userAgentData.platform is populated');

      // A provider that ships its own stealth must not be double-patched:
      // their patches plus ours contradict each other, and a contradiction is
      // a stronger signal than either alone. Asserted on the driver rather
      // than the page, because both drivers attach to the same page target
      // here and would otherwise read each other's work.
      const d3 = await connect(endpoint, { provider: 'browserbase', fingerprint: fp });
      try {
        await d3.send('navigate', { url: siteUrl });
        assert(d3.userAgent === undefined, 'a provider that ships its own stealth gets no user agent override');
        assert(
          await d3.evaluateMain(`typeof window.analyzePage === 'undefined'`),
          'and it still gets no product globals in the page',
        );
      } finally {
        d3.close();
      }
    } finally {
      d2?.close();
      await personaBrowser.close();
    }
  }
} catch (e) {
  console.log(`  ❌ threw: ${e.message}`);
  failed++;
} finally {
  driver?.close();
  await native.close();
  await new Promise((r) => site.close(r));
}

console.log('\n──────────────────────────────────────────────────');
console.log(`  ${passed} passed, ${failed} failed`);
console.log('──────────────────────────────────────────────────');
process.exit(failed ? 1 : 0);
